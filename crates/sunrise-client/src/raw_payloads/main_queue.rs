//! A receiving main provider reserves capacity before enqueue_work reports Queued.
use super::{
    Endpoint, Incoming,
    ingress::{Prepared, Producer, ReceiveLease, Refusal, Refused},
};
use mod_api::{__private::game::CallbackFailure, PayloadContextError};

/// The event union that carries one admitted raw payload to its ordered main turn.
/// Each side's event enum implements the constructor it uses; the other direction
/// is structurally impossible and never called.
pub trait RawEvent: Sized {
    /// A client's bare admitted payload.
    fn raw_payload(incoming: Incoming) -> Self;
    /// Take it back after a refused publication.
    fn into_raw_payload(self) -> Incoming;
    /// A hosted guest's delivery, with its physical identity and reply target.
    fn delivery(conn: Option<u32>, endpoint: Endpoint, incoming: Incoming) -> Self;
}

pub enum MainQueue<Ev> {
    Client(Producer<Ev>),
    ClientBundle(BundleWork),
    Server {
        events: Producer<Ev>,
        endpoint: Endpoint,
        conn: Option<u32>,
    },
}
type BundleWork = std::sync::Arc<std::sync::Mutex<Vec<Incoming>>>;

pub(crate) enum PreparedMain<Ev> {
    Client(Prepared<Ev>),
    ClientBundle(Box<(BundleWork, Incoming)>),
    Server(Prepared<Ev>),
}
/// Keeps failed opaque work alive until after the caller releases its epoch gate.
/// Boxing occurs only on failed publication, adding no allocation to successful publication.
pub(crate) enum FailedMain<Ev> {
    Client(Box<Refused<Ev>>),
    Server(Box<Refused<Ev>>),
}
impl<Ev> FailedMain<Ev> {
    pub fn reason(&self) -> Refusal {
        match self {
            Self::Client(error) => error.reason,
            Self::Server(error) => error.reason,
        }
    }
}
impl<Ev: RawEvent> MainQueue<Ev> {
    pub(crate) fn prepare(
        &self,
        incoming: Incoming,
        credit: Option<ReceiveLease<Ev>>,
    ) -> Result<PreparedMain<Ev>, (PayloadContextError, Option<CallbackFailure>)> {
        let refusal = |reason| match reason {
            Refusal::Pressure => PayloadContextError::Backpressure,
            Refusal::Closed => PayloadContextError::Disconnected,
        };
        match self {
            Self::Client(events) => {
                let event = Ev::raw_payload(incoming);
                match credit {
                    Some(credit) => events.prepare_replacing(event, credit),
                    None => events.prepare(event),
                }
                .map(PreparedMain::Client)
                .map_err(|error| {
                    let mut incoming = error.value.into_raw_payload();
                    (refusal(error.reason), incoming.dispose_body().err())
                })
            }
            Self::ClientBundle(work) => {
                // Incoming (including enqueued work) already holds its payload
                // lease. This root stays inside the one charged bundle turn.
                Ok(PreparedMain::ClientBundle(Box::new((
                    work.clone(),
                    incoming,
                ))))
            }
            Self::Server {
                events,
                endpoint,
                conn,
            } => events
                .prepare(Ev::delivery(*conn, endpoint.clone(), incoming))
                .map(PreparedMain::Server)
                .map_err(|error| {
                    let mut incoming = error.value.into_raw_payload();
                    (refusal(error.reason), incoming.dispose_body().err())
                }),
        }
    }
}
impl<Ev> PreparedMain<Ev> {
    pub(crate) fn publish(self) -> Result<(), FailedMain<Ev>> {
        match self {
            Self::Client(prepared) => prepared
                .publish()
                .map_err(|error| FailedMain::Client(Box::new(error))),
            Self::ClientBundle(work) => {
                let (pending, incoming) = *work;
                pending.lock().unwrap().push(incoming);
                Ok(())
            }
            Self::Server(prepared) => prepared
                .publish()
                .map_err(|error| FailedMain::Server(Box::new(error))),
        }
    }
}
