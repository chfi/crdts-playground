use serde::{Deserialize, Serialize};

use serde_json;

use crdts::{
    ctx::{AddCtx, ReadCtx, RmCtx},
    map::Op,
    Actor, Causal, CmRDT, CvRDT, FunkyCmRDT, FunkyCvRDT, LWWReg, Map, Orswot,
    VClock,
};

use bstr::{ByteSlice, ByteVec};

pub enum ClientCommand {
    GetDocument,
}
