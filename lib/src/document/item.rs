use serde::{Deserialize, Serialize};

use serde_json;

use crdts::{
    ctx::{AddCtx, ReadCtx, RmCtx},
    map::Op,
    Actor, Causal, CmRDT, CvRDT, FunkyCmRDT, FunkyCvRDT, LWWReg, Map, Orswot,
    VClock,
};

use bstr::{ByteSlice, ByteVec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Item<T> {
    Multiple(Vec<T>),
    Single(T),
}
