use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteConfig {
    pub id: Option<i64>,
    pub callsign: String,
    pub class: String,
    pub section: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contact {
    pub id: Option<i64>,
    pub date: String,
    pub time: String,
    pub call: String,
    pub band: String,
    pub mode: String,
    pub class: String,
    pub section: String,
    pub operator: String,
    // N1MM GUID — omitted from JSON when absent so the web UI is unaffected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n1mm_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewContact {
    pub call: String,
    pub band: String,
    pub mode: String,
    pub class: String,
    pub section: String,
    pub operator: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse { success: true, data: Some(data), error: None }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        ApiResponse { success: false, data: None, error: Some(msg.into()) }
    }
}
