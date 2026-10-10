use serde::Serialize;

/// User-visible engine failures. Messages are Vietnamese because the UI shows them.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum EngineError {
    #[error("Phụ thuộc tạo thành vòng lặp")]
    DependencyCycle,
    #[error("Không tìm thấy công việc")]
    TaskNotFound,
    #[error("Không tìm thấy nguồn lực")]
    ResourceNotFound,
    #[error("Không tìm thấy phân công")]
    AssignmentNotFound,
    #[error("Tiền nhiệm không hợp lệ: {0}")]
    BadPredecessor(String),
    #[error("Lịch không có thời gian làm việc")]
    NoWorkingTime,
    #[error("Không đọc được tệp: {0}")]
    Store(String),
    #[error("XML MSPDI không hợp lệ: {0}")]
    Xml(String),
}
