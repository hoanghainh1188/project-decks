use std::fmt;

use serde::Serialize;

use crate::lang::Lang;
use crate::recurrence::MAX_OCCURRENCES;

/// User-visible engine failures. `message` gives the text in the user's language;
/// `Display` gives the Vietnamese one.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum EngineError {
    DependencyCycle,
    TaskNotFound,
    ResourceNotFound,
    AssignmentNotFound,
    /// The predecessor text the user typed.
    BadPredecessor(String),
    ActualFinishBeforeStart,
    NoStatusDate,
    NoWorkingTime,
    /// A file could not be read. The detail comes from the system or SQLite.
    Store(String),
    /// A file could not be written. The detail comes from the system or SQLite.
    Save(String),
    NewerFormat(u32),
    /// The detail comes from the XML parser.
    Xml(String),
    /// The `version` row of a file is not a number.
    BadFormatVersion(String),
    /// No free name for the temporary file beside the file being saved.
    TempFile,
    /// The autosave slot is empty.
    NoAutosave,
    /// An edit the engine refuses, with the reason.
    Invalid(Invalid),
}

/// Why an edit was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Invalid {
    NoSuchCalendar,
    EmptyCalendarName,
    WeekNeedsSevenDays,
    BadDate,
    DateRangeTooLong,
    BadTimeRange,
    OverlappingTimeRanges,
    NotRecurring,
    OccurrenceOutline,
    OvertimeNeedsAssignment,
    DayOfMonth,
    Month,
    WeekOfMonth,
    Weekday,
    TooManyOccurrences,
    OccurrenceCount,
    SeriesEndsBeforeStart,
    NoWeekday,
    NoOccurrences,
    UnitsNotPositive,
    BadDuration,
    SplitOutsideWork,
    SplitSummary,
    SplitNoDuration,
    SplitGapNotPositive,
    NotSplit,
    NoSuchSplit,
    FirstSegment,
    NothingToPaste,
}

impl EngineError {
    /// The message to show the user, in `lang`.
    pub fn message(&self, lang: Lang) -> String {
        match (self, lang) {
            (Self::DependencyCycle, Lang::Vi) => "Phụ thuộc tạo thành vòng lặp".into(),
            (Self::DependencyCycle, Lang::En) => "The links between tasks form a loop".into(),
            (Self::TaskNotFound, Lang::Vi) => "Không tìm thấy công việc".into(),
            (Self::TaskNotFound, Lang::En) => "Task not found".into(),
            (Self::ResourceNotFound, Lang::Vi) => "Không tìm thấy nguồn lực".into(),
            (Self::ResourceNotFound, Lang::En) => "Resource not found".into(),
            (Self::AssignmentNotFound, Lang::Vi) => "Không tìm thấy phân công".into(),
            (Self::AssignmentNotFound, Lang::En) => "Assignment not found".into(),
            (Self::BadPredecessor(text), Lang::Vi) => format!("Tiền nhiệm không hợp lệ: {text}"),
            (Self::BadPredecessor(text), Lang::En) => format!("Invalid predecessor: {text}"),
            (Self::ActualFinishBeforeStart, Lang::Vi) => {
                "Ngày kết thúc thực tế phải sau ngày bắt đầu thực tế".into()
            }
            (Self::ActualFinishBeforeStart, Lang::En) => {
                "The actual finish must be after the actual start".into()
            }
            (Self::NoStatusDate, Lang::Vi) => "Chưa đặt ngày trạng thái".into(),
            (Self::NoStatusDate, Lang::En) => "The status date is not set".into(),
            (Self::NoWorkingTime, Lang::Vi) => "Lịch không có thời gian làm việc".into(),
            (Self::NoWorkingTime, Lang::En) => "The calendar has no working time".into(),
            (Self::Store(detail), Lang::Vi) => format!("Không đọc được tệp: {detail}"),
            (Self::Store(detail), Lang::En) => format!("Could not read the file: {detail}"),
            (Self::Save(detail), Lang::Vi) => format!("Không lưu được tệp: {detail}"),
            (Self::Save(detail), Lang::En) => format!("Could not save the file: {detail}"),
            (Self::NewerFormat(version), Lang::Vi) => format!(
                "Tệp được tạo bởi phiên bản mới hơn của Sheafly (định dạng {version}). \
                 Hãy cập nhật ứng dụng."
            ),
            (Self::NewerFormat(version), Lang::En) => format!(
                "This file was created by a newer version of Sheafly (format {version}). \
                 Please update the app."
            ),
            (Self::Xml(detail), Lang::Vi) => format!("XML MSPDI không hợp lệ: {detail}"),
            (Self::Xml(detail), Lang::En) => format!("Invalid MSPDI XML: {detail}"),
            (Self::BadFormatVersion(stored), Lang::Vi) => {
                format!("Không đọc được tệp: phiên bản định dạng không hợp lệ: {stored}")
            }
            (Self::BadFormatVersion(stored), Lang::En) => {
                format!("Could not read the file: invalid format version: {stored}")
            }
            (Self::TempFile, Lang::Vi) => "Không lưu được tệp: không tạo được tệp tạm".into(),
            (Self::TempFile, Lang::En) => {
                "Could not save the file: could not create a temporary file".into()
            }
            (Self::NoAutosave, Lang::Vi) => "Không đọc được tệp: không có bản tự lưu".into(),
            (Self::NoAutosave, Lang::En) => "Could not read the file: there is no autosave".into(),
            (Self::Invalid(reason), _) => reason.message(lang),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(Lang::Vi))
    }
}

impl std::error::Error for EngineError {}

impl From<Invalid> for EngineError {
    fn from(reason: Invalid) -> Self {
        Self::Invalid(reason)
    }
}

impl Invalid {
    pub fn message(self, lang: Lang) -> String {
        let (vi, en) = self.texts();
        lang.pick(vi, en)
    }

    fn texts(self) -> (String, String) {
        let fixed = |vi: &str, en: &str| (vi.to_string(), en.to_string());
        match self {
            Self::NoSuchCalendar => fixed("Không có lịch này", "That calendar does not exist"),
            Self::EmptyCalendarName => fixed("Tên lịch trống", "The calendar name is empty"),
            Self::WeekNeedsSevenDays => {
                fixed("Tuần làm việc phải có 7 ngày", "A work week must have 7 days")
            }
            Self::BadDate => fixed("Ngày không hợp lệ", "Invalid date"),
            Self::DateRangeTooLong => fixed("Khoảng ngày quá dài", "The date range is too long"),
            Self::BadTimeRange => fixed("Khoảng giờ không hợp lệ", "Invalid working time range"),
            Self::OverlappingTimeRanges => fixed(
                "Các khoảng giờ bị chồng nhau",
                "The working time ranges overlap",
            ),
            Self::NotRecurring => fixed(
                "Công việc này không phải chuỗi lặp",
                "This task is not a recurring task",
            ),
            Self::OccurrenceOutline => fixed(
                "Không đổi cấp một lần lặp; hãy sửa cả chuỗi",
                "You cannot indent or outdent a single occurrence; change the whole \
                 recurring task",
            ),
            Self::OvertimeNeedsAssignment => fixed(
                "Hãy gán nguồn lực trước khi nhập giờ làm thêm",
                "Assign a resource before entering overtime work",
            ),
            Self::DayOfMonth => fixed(
                "Ngày trong tháng phải từ 1 đến 31",
                "The day of the month must be from 1 to 31",
            ),
            Self::Month => fixed("Tháng phải từ 1 đến 12", "The month must be from 1 to 12"),
            Self::WeekOfMonth => fixed(
                "Tuần trong tháng phải là thứ nhất đến thứ tư hoặc cuối cùng",
                "The week of the month must be the first, second, third, fourth, or last",
            ),
            Self::Weekday => fixed("Thứ trong tuần không hợp lệ", "Invalid day of the week"),
            Self::TooManyOccurrences => (
                format!("Chuỗi lặp vượt quá {MAX_OCCURRENCES} lần"),
                format!("A recurring task cannot have more than {MAX_OCCURRENCES} occurrences"),
            ),
            Self::OccurrenceCount => (
                format!("Số lần lặp phải từ 1 đến {MAX_OCCURRENCES}"),
                format!("The number of occurrences must be from 1 to {MAX_OCCURRENCES}"),
            ),
            Self::SeriesEndsBeforeStart => fixed(
                "Ngày kết thúc chuỗi phải sau ngày bắt đầu",
                "The recurrence must end after it starts",
            ),
            Self::NoWeekday => fixed(
                "Hãy chọn ít nhất một thứ trong tuần",
                "Select at least one day of the week",
            ),
            Self::NoOccurrences => fixed(
                "Chuỗi lặp không có lần nào",
                "The recurring task has no occurrences",
            ),
            Self::UnitsNotPositive => fixed(
                "Đơn vị nguồn lực phải lớn hơn 0",
                "Assignment units must be greater than 0",
            ),
            Self::BadDuration => fixed("Thời lượng không hợp lệ", "Invalid duration"),
            Self::SplitOutsideWork => fixed(
                "Điểm tách phải nằm trong thời gian làm việc của công việc",
                "The split must fall within the task's working time",
            ),
            Self::SplitSummary => fixed(
                "Không tách được công việc tóm tắt",
                "A summary task cannot be split",
            ),
            Self::SplitNoDuration => fixed(
                "Công việc không có thời lượng nên không tách được",
                "A task with no duration cannot be split",
            ),
            Self::SplitGapNotPositive => fixed(
                "Khoảng ngắt phải lớn hơn 0",
                "The split length must be greater than 0",
            ),
            Self::NotSplit => fixed("Công việc này chưa bị tách", "This task is not split"),
            Self::NoSuchSplit => fixed("Không có khoảng ngắt này", "That split does not exist"),
            Self::FirstSegment => fixed(
                "Đoạn đầu dời cùng cả công việc; hãy kéo cả thanh",
                "The first segment moves with the whole task; drag the bar instead",
            ),
            Self::NothingToPaste => fixed("Không có gì để dán", "There is nothing to paste"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_stays_vietnamese() {
        assert_eq!(EngineError::TaskNotFound.to_string(), "Không tìm thấy công việc");
        assert_eq!(
            EngineError::Invalid(Invalid::NoSuchCalendar).to_string(),
            "Không có lịch này"
        );
    }

    #[test]
    fn every_reason_has_both_languages() {
        use Invalid::*;
        let all = [
            NoSuchCalendar,
            EmptyCalendarName,
            WeekNeedsSevenDays,
            BadDate,
            DateRangeTooLong,
            BadTimeRange,
            OverlappingTimeRanges,
            NotRecurring,
            OccurrenceOutline,
            OvertimeNeedsAssignment,
            DayOfMonth,
            Month,
            WeekOfMonth,
            Weekday,
            TooManyOccurrences,
            OccurrenceCount,
            SeriesEndsBeforeStart,
            NoWeekday,
            NoOccurrences,
            UnitsNotPositive,
            BadDuration,
            SplitOutsideWork,
            SplitSummary,
            SplitNoDuration,
            SplitGapNotPositive,
            NotSplit,
            NoSuchSplit,
            FirstSegment,
            NothingToPaste,
        ];
        for reason in all {
            let (vi, en) = (reason.message(Lang::Vi), reason.message(Lang::En));
            assert!(!vi.is_empty() && !en.is_empty(), "{reason:?}");
            assert_ne!(vi, en, "{reason:?}");
            assert!(en.is_ascii(), "{reason:?}: {en}");
        }
    }
}
