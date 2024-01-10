use radars::pointcloud::PointCloud;
use std::time::Instant;

#[derive(Debug)]
pub struct FrameBuffer {
    pub finished: PointCloud,
    pub filling: PointCloud,
    pub timer: Instant,
    pub buffer_duration: u128,
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self {
            finished: PointCloud::default(),
            filling: PointCloud::default(),
            timer: Instant::now(),
            buffer_duration: 100,
        }
    }
}

impl FrameBuffer {
    pub fn new(buffer_size: u128) -> Self {
        Self {
            buffer_duration: buffer_size,
            ..Default::default()
        }
    }

    pub fn push_frame(&mut self, frame: &mut PointCloud) {
        self.reorganize();
        self.filling.extend(frame);
    }

    pub fn reorganize(&mut self) {
        if self.timer.elapsed().as_millis() <= self.buffer_duration {
            return;
        }
        std::mem::swap(&mut self.filling, &mut self.finished);
        self.filling = PointCloud::default();
        self.timer = Instant::now();
    }

    pub fn get(&mut self) -> PointCloud {
        self.reorganize();
        self.finished.clone()
    }
}
