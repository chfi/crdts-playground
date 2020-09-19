use serde::{Deserialize, Serialize};

use serde_json;

use crdts::{
    ctx::{AddCtx, ReadCtx, RmCtx},
    map::Op,
    Actor, Causal, CmRDT, CvRDT, FunkyCmRDT, FunkyCvRDT, LWWReg, Map, Orswot,
    VClock,
};

use bstr::{ByteSlice, ByteVec};

pub type DocActor = u32;
pub type RecordKey = u64;
pub type RecordEntry = u8;
pub type Record = Orswot<RecordEntry, DocActor>;
pub type RecordMap = Map<u64, Orswot<RecordEntry, DocActor>, DocActor>;

pub type DocumentOp = Op<u64, Orswot<u8, u32>, u32>;
pub type RecordOp = crdts::orswot::Op<RecordEntry, DocActor>;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
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

impl Command {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn parse_input(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.trim();
        match bytes {
            b"get_document" => Some(Self::GetDocument),
            b"get_q1" => Some(Self::GetRecord { key: "q1".into() }),
            b"get_q2" => Some(Self::GetRecord { key: "q2".into() }),
            b"get_q3" => Some(Self::GetRecord { key: "q3".into() }),
            b"get_readctx" => Some(Self::GetReadCtx),
            b"request_actor" => Some(Self::RequestActor),
            bs => {
                let mut fields = bs.split_str(b":");

                let next_field = fields.next()?;
                let actor = std::str::from_utf8(next_field).ok()?;
                let actor = actor.parse::<u32>().ok()?;

                let next_field = fields.next()?;
                let key = std::str::from_utf8(next_field).ok()?;
                let key = key.to_string();

                let next_field = fields.next()?;
                let content = std::str::from_utf8(next_field).ok()?;
                let content = content.parse::<u8>().ok()?;

                Some(Self::Update {
                    actor,
                    key,
                    content,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub records: RecordMap,
}

impl Document {
    fn new() -> Self {
        Default::default()
    }

    pub fn example() -> Self {
        let records: Map<u64, Orswot<u8, u32>, u32> = Map::new();
        // let read_ctx: ReadCtx<usize, u32> = records.len();

        let mut doc = Document { records };

        let read_ctx = doc.get_read_ctx();
        doc.update_record(1, read_ctx.derive_add_ctx(0), |set, ctx| {
            let items: &[u8] = &[0, 1, 2, 3, 4];
            set.add_all(items.into_iter().copied(), ctx)
        });

        /*
        let ops = vec![
            records.update(1 as u64, read_ctx.derive_add_ctx(0), |set, ctx| {
                set.add(0, ctx)
            }),
            records.update(2 as u64, read_ctx.derive_add_ctx(0), |set, ctx| {
                set.add(0, ctx)
            }),
            records.update(3 as u64, read_ctx.derive_add_ctx(0), |set, ctx| {
                set.add(0, ctx)
            }),
        ];

        ops.into_iter().for_each(|op| records.apply(op));


        Document { records }
        */
    }

    pub fn update_record<F>(
        &mut self,
        key: u64,
        ctx: AddCtx<DocActor>,
        f: F,
    ) -> DocumentOp
    where
        F: FnOnce(&Record, AddCtx<DocActor>) -> RecordOp,
    {
        self.records.update(key, ctx, f)
    }

    pub fn get_read_ctx(&self) -> ReadCtx<(), u32> {
        self.records.read_ctx()
    }

    pub fn get_record(&self, key: u64) -> ReadCtx<Option<Record>, DocActor> {
        self.records.get(&key)
    }

    pub fn apply(&mut self, op: DocumentOp) {
        self.records.apply(op)
    }

    pub fn doc_keys(&self) -> impl Iterator<Item = ReadCtx<&u64, DocActor>> {
        self.records.keys()
    }

    pub fn to_json_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
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
