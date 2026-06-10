use super::operand::LabelId;

#[derive(Debug)]
pub struct LabelManager {
    next_id: usize,
}

impl LabelManager {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn new_label(&mut self) -> LabelId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn label_name(id: LabelId) -> String {
        format!("L{}", id)
    }

    pub fn reset(&mut self) {
        self.next_id = 0;
    }

    pub fn count(&self) -> usize {
        self.next_id
    }
}

impl Default for LabelManager {
    fn default() -> Self {
        Self::new()
    }
}
