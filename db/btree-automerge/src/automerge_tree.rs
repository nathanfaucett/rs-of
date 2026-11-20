use core::{fmt, ops::RangeBounds};
use std::sync::Arc;

use async_lock::RwLock;
use async_stream::stream;
use automerge::AutoCommit;
use futures::{Stream, StreamExt, pin_mut};

use btree::{BTree, BTreeRead, BTreeResult};

use crate::{
    AutomergeBTreeTransaction, DocumentChangeKey, ThresholdPolicy, document_change_key::DocumentId,
    reconstruction::reconstruct_documents, util::tx_get_document,
};

#[derive(Clone, Debug)]
pub struct AutomergeBTree<B>
where
    B: BTree<DocumentChangeKey, Vec<u8>> + fmt::Debug,
{
    inner: Arc<RwLock<B>>,
    policy: ThresholdPolicy,
}

impl<B> AutomergeBTree<B>
where
    B: BTree<DocumentChangeKey, Vec<u8>> + fmt::Debug,
{
    pub fn new(inner: B) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            policy: ThresholdPolicy::default(),
        }
    }
    pub fn with_compaction(inner: B, threshold_count: usize, threshold_bytes: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
            policy: ThresholdPolicy {
                threshold_count,
                threshold_bytes,
            },
        }
    }
}

impl<B> BTreeRead<DocumentId, AutoCommit> for AutomergeBTree<B>
where
    B: BTree<DocumentChangeKey, Vec<u8>> + fmt::Debug,
{
    async fn get(&self, key: &DocumentId) -> BTreeResult<Option<AutoCommit>> {
        let inner_guard = self.inner.read().await;
        let mut tx = inner_guard.transaction().await?;
        let document = tx_get_document(&mut tx, key, false, &self.policy).await?;
        drop(tx);
        drop(inner_guard);
        Ok(document.map(|(document, _)| document))
    }

    fn range<R>(&self, range: R) -> impl Stream<Item = BTreeResult<(DocumentId, AutoCommit)>>
    where
        R: RangeBounds<DocumentId> + Clone,
    {
        stream!({
            let inner_range = DocumentChangeKey::map_document_id_range(range);

            let inner_guard = self.inner.read().await;
            {
                let documents = reconstruct_documents(inner_guard.range(inner_range));
                pin_mut!(documents);

                while let Some(document) = documents.next().await {
                    match document.result {
                        Ok(mut document) => {
                            if let Some(doc) = document.doc.take() {
                                yield Ok((document.id, doc));
                            }
                        }
                        Err(error) => yield Err(error),
                    }
                }
            }
            drop(inner_guard);
        })
    }
}

impl<B> BTree<DocumentId, AutoCommit> for AutomergeBTree<B>
where
    B: BTree<DocumentChangeKey, Vec<u8>> + fmt::Debug,
{
    type Transaction = AutomergeBTreeTransaction<B::Transaction>;

    async fn transaction(&self) -> BTreeResult<Self::Transaction> {
        let inner_guard = self.inner.read().await;
        let inner_tx = inner_guard.transaction().await?;
        drop(inner_guard);
        Ok(AutomergeBTreeTransaction::new(
            inner_tx,
            self.policy.clone(),
        ))
    }
}
