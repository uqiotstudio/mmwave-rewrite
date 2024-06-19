use crate::core::pointcloud::PointCloud;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Accumulator {
    blocks: HashMap<u128, Vec<PointCloud>>,
    finished: VecDeque<PointCloud>,
    threshold: u128, // Threshold in ms before a block is complete
    peeked: bool,
}

impl fmt::Debug for Accumulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("TimeBlockMap");

        // Add custom formatting for 'blocks'
        // debug_struct.field("blocks", &self.blocks);
        debug_struct.field(
            "blocks",
            &self
                .blocks
                .iter()
                .map(|(k, v)| (k, v.iter().map(|p| p.time).collect::<Vec<_>>()))
                .collect::<HashMap<_, _>>(),
        );

        // Add custom formatting for 'd'
        debug_struct.field("finished", &self.finished.len());

        // Add 'threshold' field
        debug_struct.field("threshold", &self.threshold);

        debug_struct.finish()
    }
}

impl Accumulator {
    pub fn new(threshold: u128) -> Self {
        Accumulator {
            blocks: HashMap::new(),
            finished: VecDeque::new(),
            threshold,
            peeked: false,
        }
    }

    pub fn push(&mut self, item: PointCloud) {
        if self.is_item_within_threshold(&item) {
            let block = self.get_block_for_item(&item);
            self.blocks.entry(block).or_insert_with(Vec::new).push(item);
        }
    }

    pub fn push_multiple(&mut self, items: Vec<PointCloud>) {
        for item in items {
            self.push(item);
        }
    }

    fn get_block_for_item(&self, item: &PointCloud) -> u128 {
        item.time - (item.time % 100)
    }

    fn is_item_within_threshold(&self, item: &PointCloud) -> bool {
        !self.is_block_old(self.get_block_for_item(&item))
    }

    fn is_block_old(&self, block: u128) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|t| t.as_millis())
            .unwrap_or(0);

        (current_time > block) && (current_time - block > self.threshold)
    }

    pub fn reorganize(&mut self) {
        let old_blocks = self
            .blocks
            .keys()
            .cloned()
            .filter(|&k| self.is_block_old(k))
            .collect::<Vec<_>>();

        for block in old_blocks {
            if let Some(items) = self.blocks.remove(&block) {
                let merged_item = items.into_iter().reduce(|mut a, b| {
                    a.extend(b);
                    a
                });
                if let Some(mut item) = merged_item {
                    item.time = block;
                    self.finished.push_back(item);
                    self.peeked = false;
                }
            }
        }
    }

    pub fn pop_finished(&mut self) -> VecDeque<PointCloud> {
        let mut replacement = VecDeque::new();
        std::mem::swap(&mut replacement, &mut self.finished);
        replacement
    }

    pub fn peekable(&self) -> bool {
        !self.peeked
    }

    pub async fn peek(&mut self) -> Option<PointCloud> {
        while !self.peekable() {
            // Recheck every 100ms
            tokio::time::sleep(Duration::from_millis(100));
        }
        self.peeked = true;
        self.finished.back().cloned()
    }

    /// Returns the front of finished, which is the oldest item stored
    pub async fn pop_single_finished(&mut self) -> Option<PointCloud> {
        self.finished.pop_front()
    }
}
