use super::operand::TempId;

#[derive(Debug)]
pub struct TempManager {
    next_id: usize,
}

impl TempManager {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn new_temp(&mut self) -> TempId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn temp_name(id: TempId) -> String {
        format!("t{}", id)
    }

    pub fn reset(&mut self) {
        self.next_id = 0;
    }

    pub fn count(&self) -> usize {
        self.next_id
    }
}

impl Default for TempManager {
    fn default() -> Self {
        Self::new()
    }
}
