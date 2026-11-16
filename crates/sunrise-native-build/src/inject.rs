//! Raw function declarations and the staged point catalog.
use std::{fs, path::Path};

/// A game build's point, as its `points.json` lists it.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ListedPoint {
    pub name: String,
    pub target: String,
    pub at_return: bool,
    /// Why its API listeners are refused, naming the parameter with no API type.
    #[serde(default)]
    pub api_refusal: Option<String>,
    /// What its listeners may do with the return: `cancel`, `result` or `none`.
    #[serde(default)]
    pub returns: String,
    /// Its API listeners' host: `none`, or the host they take.
    #[serde(default)]
    pub host: String,
    /// The function's return type as written, which a `replace` at a `cancel` point needs `()`.
    #[serde(default)]
    pub return_ty: Option<String>,
}

impl ListedPoint {
    /// The point as the shared API constructor emitter reads it.
    pub fn emitted(&self) -> sunrise_mod_point_emit::Point {
        sunrise_mod_point_emit::Point {
            name: self.name.clone(),
            target: self.target.clone(),
            at_return: self.at_return,
            api_refusal: self.api_refusal.clone(),
            returns: self.returns.clone(),
            host: self.host.clone(),
            return_ty: self.return_ty.clone(),
        }
    }
}

/// The `points.json` the game's build script writes, beside the prebuilt game or in the
/// staged game's build output.
pub const POINTS: &str = "points.json";

#[derive(serde::Deserialize)]
pub struct Catalog {
    pub points: Vec<ListedPoint>,
}

pub fn catalog(path: &Path) -> Result<Catalog, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (_, catalog) = sunrise_mod_package::build_data::read(POINTS, &text)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(catalog)
}
