use serde::{Deserialize, Serialize};

use crdts::{
    ctx::{AddCtx, ReadCtx, RmCtx},
    map::Op,
    Actor, Causal, CmRDT, CvRDT, FunkyCmRDT, FunkyCvRDT, LWWReg, Map, Orswot,
    VClock,
};

type DocActor = u32;
type RecordEntry = u8;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Command {
    GetDocument,
    GetRecord {
        key: String,
    },
    GetReadCtx,
    Update {
        actor: DocActor,
        key: String,
        content: RecordEntry,
    },
    RequestActor,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub records: Map<String, Orswot<RecordEntry, DocActor>, DocActor>,
}

type DocumentOp = Op<String, Orswot<u8, u32>, u32>;

impl Document {
    fn new() -> Self {
        Default::default()
    }

    pub fn example() -> Self {
        let mut records: Map<String, Orswot<u8, u32>, u32> = Map::new();
        let read_ctx: ReadCtx<usize, u32> = records.len();

        let ops = vec![
            records.update(
                String::from("q1"),
                read_ctx.derive_add_ctx(0),
                |set, ctx| set.add(0, ctx),
            ),
            records.update(
                String::from("q2"),
                read_ctx.derive_add_ctx(0),
                |set, ctx| set.add(0, ctx),
            ),
            records.update(
                String::from("q3"),
                read_ctx.derive_add_ctx(0),
                |set, ctx| set.add(0, ctx),
            ),
        ];

        ops.into_iter().for_each(|op| records.apply(op));

        Document { records }
    }

    pub fn get_read_ctx(&self) -> ReadCtx<(), u32> {
        self.records.read_ctx()
    }

    pub fn apply(&mut self, op: DocumentOp) {
        self.records.apply(op)
    }
}

/*
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u32,
    pub docs: Docs,
}

impl User {
    pub fn new(id: u32) -> Self {
        User {
            id,
            docs: Default::default(),
        }
    }
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
