//! Recovered logging projection; inventory delivery and persistence are separate effects.
use super::legacy_user_data::{LegacyBytes, SqlDateTime2};
use thiserror::Error;
#[derive(Debug, Error)]
#[error("item log projection exceeds recovered bounds")]
pub struct LogBound;
pub struct Data {
    pub item_id: i64,
    class: LegacyBytes<1024, true>,
    pub count: i32,
    pub colors: [i32; 3],
    pub reduced: i32,
    pub bonus: i32,
    pub system: bool,
}
impl Data {
    pub fn new(
        item_id: i64,
        class: Vec<u8>,
        count: i32,
        colors: [i32; 3],
        reduced: i32,
        bonus: i32,
        system: bool,
    ) -> Result<Self, LogBound> {
        Ok(Self {
            item_id,
            class: LegacyBytes::from_bytes(class).map_err(|_| LogBound)?,
            count,
            colors,
            reduced,
            bonus,
            system,
        })
    }
    /// From(owner,string,num), not a generated Item or assigned identifier.
    pub fn requested(class: Vec<u8>, count: i32) -> Result<Self, LogBound> {
        Self::new(-1, class, count, [-1; 3], 0, 0, false)
    }
    pub fn class(&self) -> &[u8] {
        self.class.as_bytes()
    }
}
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub inventory: Option<i32>,
    pub shared: Option<i32>,
}
pub struct Record {
    pub journal: i64,
    pub event: i16,
    pub detail: i16,
    pub before_id: Option<i64>,
    pub data: Data,
    pub after: Counts,
    pub memo: Option<LegacyBytes<1024, true>>,
    pub captured: SqlDateTime2,
}
pub struct Manager {
    journal: i64,
    event: i16,
    records: Vec<Record>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            journal: -1,
            event: 0,
            records: Vec::new(),
        }
    }
}
impl Manager {
    pub fn set_journal(&mut self, journal: i64, event: i16) {
        self.journal = journal;
        self.event = event;
    }
    pub fn clear(&mut self) {
        self.records.clear();
    }
    pub fn pending(&self) -> usize {
        self.records.len()
    }
    pub fn add(
        &mut self,
        data: Data,
        enabled: bool,
        now: impl FnOnce() -> SqlDateTime2,
    ) -> Result<(), LogBound> {
        if !enabled {
            return Ok(());
        }
        let detail = match (data.system, data.count < 0) {
            (true, true) => 6,
            (true, false) => 5,
            (false, true) => 3,
            (false, false) => 2,
        };
        self.add_detail(data, detail, true, now)
    }
    pub fn add_detail(
        &mut self,
        data: Data,
        detail: i16,
        enabled: bool,
        now: impl FnOnce() -> SqlDateTime2,
    ) -> Result<(), LogBound> {
        if !enabled {
            return Ok(());
        }
        if self.records.len() >= 4096 {
            return Err(LogBound);
        }
        self.records.push(Record {
            journal: self.journal,
            event: self.event,
            detail,
            before_id: None,
            data,
            after: Counts::default(),
            memo: None,
            captured: now(),
        });
        Ok(())
    }
    /// Only the last record receives live owner counts. Disabled Write retains the queue.
    pub fn prepare_write(
        &mut self,
        enabled: bool,
        counts: impl FnOnce(&Data) -> Counts,
    ) -> Option<&[Record]> {
        if !enabled {
            return None;
        }
        let shared_delta = self
            .records
            .iter()
            .filter(|r| r.event == 4 && r.detail == 4)
            .fold(0i32, |v, r| v.wrapping_add(r.data.count));
        if let Some(last) = self.records.last_mut() {
            last.after = counts(&last.data);
            last.after.shared = last.after.shared.map(|v| v.wrapping_add(shared_delta));
        }
        Some(&self.records)
    }
}
pub struct Ledger {
    pub journal: i64,
    pub event: i16,
    pub action: i16,
    pub data: Data,
    pub bundle: Option<i32>,
}
impl Ledger {
    pub fn creation(journal: i64, data: Data, bundle: Option<i32>) -> Self {
        Self {
            journal,
            event: 2,
            action: 23,
            data,
            bundle,
        }
    }
    /// Original SimpleLogData.From(str) -> AddSimpleLog(OperationFail,CreateCharacter).
    pub fn creation_failure(journal: i64, message: Vec<u8>) -> Result<Self, LogBound> {
        Ok(Self {
            journal,
            event: 0,
            action: 23,
            data: Data::new(-1, message, 0, [0; 3], 0, 0, false)?,
            bundle: None,
        })
    }
    pub fn suppressed(&self, disable_logging: bool) -> bool {
        disable_logging || self.journal == -1 || self.event == 18
    }
}
