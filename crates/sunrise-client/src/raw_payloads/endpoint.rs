//! Physical reply targets usable by either receiving executor without borrowing Game.
//!
//! The endpoint knows the raw-payload rules (authority, size limits, droppable
//! queues, framing preflight) but nothing about who owns the other end: a solo
//! client session and a hosted guest both implement [`Output`].
use super::{Side, Transport, connection::Epoch, encode, encoded_len, pairing::PairingPayload};
use mod_api::{PayloadBytes, PayloadFrameStage, PayloadId, PayloadSendError, Text};
use std::sync::{Arc, Weak};
use sunrise_protocol::{connection::Framing, frame, payload_negotiation::Phase};

/// The write side of one physical connection: where a prepared reply goes.
pub trait Output: Send + Sync {
    fn transport(&self) -> &Arc<Transport>;
    fn side(&self) -> Side;
    /// Frame limits and backpressure capacity for a send of `id` in `phase`, or the
    /// refusal when this output no longer accepts that phase.
    fn limits(&self, phase: Phase, id: &PayloadId) -> Result<(Framing, usize), PayloadSendError>;
    /// Reserve an already-framed body; the result publishes it after the handler commits.
    fn prepare(
        &self,
        body: Vec<u8>,
        id: &PayloadId,
    ) -> Result<Box<dyn PreparedSend>, PayloadSendError>;
    /// Close this physical connection for `reason`.
    fn disconnect(&self, reason: &crate::text::Text);
    /// Per-endpoint diagnostics for the payload probe, never retaining the owner.
    #[cfg(any(test, feature = "test-payload-probe"))]
    fn diagnostics(&self) -> Box<dyn Fn() -> String + Send + Sync>;
}

/// A reserved outgoing frame: publishing it is the only thing a reply can do.
pub trait PreparedSend: Send {
    fn publish(self: Box<Self>) -> Result<(), PayloadSendError>;
    /// Hand the reservation and its framed bytes to a pairing bundle. Only a
    /// hosted guest's play output frames bodies that way.
    fn into_pairing(self: Box<Self>, _epoch: Arc<Epoch>) -> Option<Arc<PairingPayload>> {
        None
    }
}

/// The configuration task a handler finishes, named without its host type.
pub trait ConfigurationTask: Send + Sync {
    fn running_ordinal_for_type(
        &self,
        task_type: &mod_api::Identifier,
    ) -> Result<usize, mod_api::ConfigurationTaskError>;
    fn finish_ordinal_under_epoch(&self, ordinal: usize);
    fn finish_type(
        &self,
        task_type: &mod_api::Identifier,
    ) -> Result<(), mod_api::ConfigurationTaskError>;
}

#[derive(Clone)]
pub struct Endpoint {
    output: Arc<dyn Output>,
    session: Option<Weak<dyn ConfigurationTask>>,
}
/// Retains no endpoint, sender, producer, writer, transport or payload lease.
#[cfg(any(test, feature = "test-payload-probe"))]
pub(super) struct Observation {
    transport: std::sync::Weak<Transport>,
    side: Side,
    diagnostics: Box<dyn Fn() -> String + Send + Sync>,
}
#[cfg(any(test, feature = "test-payload-probe"))]
impl Observation {
    pub(super) fn same_target(&self, endpoint: &Endpoint) -> bool {
        self.transport.ptr_eq(&Arc::downgrade(endpoint.transport())) && self.side == endpoint.side()
    }
    pub(super) fn diagnostics(&self) -> String {
        let connection = match self.transport.upgrade() {
            Some(transport) => transport
                .connection
                .as_ref()
                .map(|c| c.diagnostics())
                .unwrap_or_else(|| "no connection".into()),
            None => "transport owner dropped".into(),
        };
        format!("{:?}: {connection}; {}", self.side, (self.diagnostics)())
    }
}
pub enum Prepared {
    /// A reply a physical output reserved.
    Frame(Box<dyn PreparedSend>),
    /// A `Droppable` send: it reserves nothing, and publication queues it for the writer.
    Droppable(
        Arc<super::droppable::Queue>,
        Arc<mod_api::__private::game::DropCounters>,
        Vec<u8>,
    ),
}
impl Prepared {
    pub fn publish(self) -> Result<(), PayloadSendError> {
        match self {
            Self::Frame(send) => send.publish(),
            Self::Droppable(queue, counters, body) => {
                queue.push(std::time::Instant::now(), &counters, body);
                Ok(())
            }
        }
    }
    /// Extract a hosted guest's prepared reservation for inclusion inside an entity
    /// pairing bundle. Other outputs produce none.
    pub(crate) fn into_pairing(self, epoch: Arc<Epoch>) -> Option<Arc<PairingPayload>> {
        match self {
            Self::Frame(send) => send.into_pairing(epoch),
            Self::Droppable(..) => None,
        }
    }
}
impl Endpoint {
    pub fn new(output: impl Output + 'static) -> Self {
        Self {
            output: Arc::new(output),
            session: None,
        }
    }
    /// A hosted guest's play connection.
    pub fn server(output: impl Output + 'static) -> Self {
        Self::new(output)
    }
    /// A hosted guest's configuration connection, with the task session it finishes.
    pub fn configuration(
        output: impl Output + 'static,
        session: Option<Weak<dyn ConfigurationTask>>,
    ) -> Self {
        Self {
            output: Arc::new(output),
            session,
        }
    }
    pub fn configuration_session(&self) -> Option<&Weak<dyn ConfigurationTask>> {
        self.session.as_ref()
    }
    pub fn side(&self) -> Side {
        self.output.side()
    }
    pub fn transport(&self) -> &Arc<Transport> {
        self.output.transport()
    }
    #[cfg(any(test, feature = "test-payload-probe"))]
    pub(super) fn observation(&self) -> Observation {
        Observation {
            transport: Arc::downgrade(self.transport()),
            side: self.side(),
            diagnostics: self.output.diagnostics(),
        }
    }
    /// Peer authority is checked before bytes are copied. Publication belongs to the root.
    pub fn prepare(
        &self,
        epoch: &Arc<Epoch>,
        id: &PayloadId,
        bytes: PayloadBytes,
    ) -> Result<Prepared, PayloadSendError> {
        #[cfg(any(test, feature = "test-payload-probe"))]
        super::staging_probe::capture(&id.0, self);
        epoch.can_send(id)?;
        // A raw channel keeps vanilla's clientbound custom-payload limit, by name.
        let limit = sunrise_protocol::packets::custom_payload::CLIENTBOUND_LIMIT;
        if epoch.peer.local.is_raw(&id.0) && bytes.0.len() > limit {
            return Err(PayloadSendError::PayloadTooLarge {
                id: id.clone(),
                size: bytes.0.len() as u64,
                limit: limit as u64,
            });
        }
        let len = encoded_len(epoch.phase, self.side(), id, bytes.0.len());
        if epoch.peer.local.has_droppable()
            && let Some(counters) = epoch.peer.local.droppable(&id.0)
        {
            return self.prepare_droppable(epoch, id, bytes, len, counters);
        }
        let (framing, capacity) = self.output.limits(epoch.phase, id)?;
        preflight(framing, capacity, len, id)?;
        let body = encode(epoch.phase, self.side(), id, bytes);
        self.output.prepare(body, id).map(Prepared::Frame)
    }
    /// A `Droppable` send takes no reservation: it is bounded by its own queue. Frame limits
    /// still refuse here, so no waiting send can fail framing later.
    fn prepare_droppable(
        &self,
        epoch: &Arc<Epoch>,
        id: &PayloadId,
        bytes: PayloadBytes,
        len: usize,
        counters: &Arc<mod_api::__private::game::DropCounters>,
    ) -> Result<Prepared, PayloadSendError> {
        let (framing, _) = self.output.limits(epoch.phase, id)?;
        preflight(framing, usize::MAX, len, id)?;
        let queue = self
            .transport()
            .connection
            .as_ref()
            .and_then(|connection| connection.droppable())
            .ok_or(PayloadSendError::Disconnected)?;
        Ok(Prepared::Droppable(
            queue.clone(),
            counters.clone(),
            encode(epoch.phase, self.side(), id, bytes),
        ))
    }
    /// A failure closes this physical connection only; a client reason joins its ordered events.
    pub fn disconnect(&self, reason: Text) {
        let reason = crate::api_text::to_serialized_engine(&reason).unwrap_or_else(|error| {
            crate::text::Text::tr(
                "disconnect.genericReason",
                &[&format!("invalid payload disconnect reason: {error:?}")],
            )
        });
        self.transport().close();
        self.output.disconnect(&reason);
    }
}
fn preflight(
    framing: Framing,
    capacity: usize,
    len: usize,
    id: &PayloadId,
) -> Result<(), PayloadSendError> {
    let frame_limit = match framing {
        Framing::Memory => None,
        Framing::Network(None) => Some((PayloadFrameStage::Encoded, frame::MAX_FRAME)),
        Framing::Network(Some(_)) => {
            Some((PayloadFrameStage::Uncompressed, frame::MAX_UNCOMPRESSED))
        }
    };
    if let Some((stage, limit)) = frame_limit
        && len > limit
    {
        return Err(PayloadSendError::FrameTooLarge {
            id: id.clone(),
            stage,
            size: len as u64,
            limit: limit as u64,
        });
    }
    if len > capacity {
        return Err(PayloadSendError::Backpressure);
    }
    Ok(())
}
