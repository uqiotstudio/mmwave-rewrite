use super::point::Point;
use dyn_clone::DynClone;

pub trait Timestamp {
    type Time;
    fn get_timestamp(&self) -> Self::Time;
}

#[typetag::serde(tag = "DataType")]
pub trait Data: std::fmt::Debug + Send + Sync + DynClone {
    fn into_points(&self) -> Vec<Point>;
}
dyn_clone::clone_trait_object!(Data);
