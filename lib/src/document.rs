pub mod item;

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
pub type RecordKey = u32;
pub type RecordEntry = Vec<u8>;
pub type OrswotRecord = Orswot<RecordEntry, DocActor>;
pub type RecordMap = Map<u32, Orswot<RecordEntry, DocActor>, DocActor>;

// pub type DocumentOp = Op<u64, Orswot<RecordEntry, u32>, u32>;
pub type DocumentOp = Op<u32, OrswotRecord, u32>;
pub type RecordOp = crdts::orswot::Op<RecordEntry, DocActor>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    GetDocument,
    GetRecord {
        key: RecordKey,
    },
    GetReadCtx,
    Add {
        add_ctx: AddCtx<DocActor>,
        key: RecordKey,
        content: String,
    },
    Apply {
        op: DocumentOp,
    },
}

impl Command {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bincode::deserialize(bytes).ok()
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        bincode::serialize(self).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocResponse {
    Document(Document),
    Record(ReadCtx<Option<OrswotRecord>, DocActor>),
    ReadCtx(ReadCtx<(), u32>),
}

impl DocResponse {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bincode::deserialize(bytes).ok()
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        bincode::serialize(self).ok()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub records: RecordMap,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocReplica {
    doc: Document,
    actor: DocActor,
    log: Vec<DocumentOp>,
}

impl DocReplica {
    pub fn from_document(actor: DocActor, doc: &Document) -> Self {
        DocReplica {
            doc: doc.clone(),
            actor,
            log: Vec::new(),
        }
    }

    pub fn apply_op(&mut self, op: DocumentOp) {
        self.log.push(op.clone());
        self.doc.records.apply(op);
    }
}

impl Document {
    pub fn example() -> Self {
        let records: Map<u32, Orswot<Vec<u8>, u32>, u32> = Map::new();

        let mut doc = Document { records };

        let read_ctx = doc.get_read_ctx();
        let op =
            doc.update_record(1, read_ctx.derive_add_ctx(0), |set, ctx| {
                let items: Vec<Vec<u8>> = vec![
                    Vec::from("thing 1"),
                    Vec::from("another thing"),
                    Vec::from("who knows what this is"),
                ];
                set.add_all(items, ctx)
            });

        doc.apply(op);

        doc
    }

    pub fn update_record<F>(
        &mut self,
        key: u32,
        ctx: AddCtx<DocActor>,
        f: F,
    ) -> DocumentOp
    where
        F: FnOnce(&OrswotRecord, AddCtx<DocActor>) -> RecordOp,
    {
        self.records.update(key, ctx, f)
    }

    pub fn get_read_ctx(&self) -> ReadCtx<(), u32> {
        self.records.read_ctx()
    }

    pub fn get_record(
        &self,
        key: u32,
    ) -> ReadCtx<Option<OrswotRecord>, DocActor> {
        self.records.get(&key)
    }

    pub fn apply(&mut self, op: DocumentOp) {
        self.records.apply(op)
    }

    pub fn doc_keys(&self) -> impl Iterator<Item = ReadCtx<&u32, DocActor>> {
        self.records.keys()
    }

    pub fn keys_vec(&self) -> Vec<ReadCtx<&u32, DocActor>> {
        self.records.keys().collect()
    }

    pub fn to_json_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}
