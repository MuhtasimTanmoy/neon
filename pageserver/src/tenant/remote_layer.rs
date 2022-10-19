//! A RemoteLayer is an in-memory placeholder for a layer file that exists
//! in remote storage.
//!
use crate::config::PageServerConf;
use crate::repository::{Key, Value};
use crate::storage_sync::index::LayerFileMetadata;
use crate::tenant::delta_layer::DeltaLayer;
use crate::tenant::filename::{DeltaFileName, ImageFileName};
use crate::tenant::image_layer::ImageLayer;
use crate::tenant::storage_layer::{Layer, ValueReconstructResult, ValueReconstructState};
use anyhow::{bail, Result};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use utils::{
    id::{TenantId, TimelineId},
    lsn::Lsn,
};

use super::filename::LayerFileName;
use super::storage_layer::PersistentLayer;

#[derive(Debug)]
pub struct RemoteLayer {
    tenantid: TenantId,
    timelineid: TimelineId,
    key_range: Range<Key>,
    lsn_range: Range<Lsn>,

    pub file_name: LayerFileName,

    pub layer_metadata: LayerFileMetadata,

    is_delta: bool,

    is_incremental: bool,

    /// `download_watch` can be used to wait for download of the layer to finish.
    /// If it is Some, you can subscribe on the watch to be notified when the
    /// download finishes. If it's None, no download is in progress yet.
    /// See Timeline::download_remote_layer(), that is the higher-level function
    /// to coordinate layer download.
    pub download_watch: Mutex<Option<tokio::sync::watch::Sender<Result<()>>>>,
}

impl Layer for RemoteLayer {
    fn get_key_range(&self) -> Range<Key> {
        self.key_range.clone()
    }

    fn get_lsn_range(&self) -> Range<Lsn> {
        self.lsn_range.clone()
    }

    fn get_value_reconstruct_data(
        &self,
        _key: Key,
        _lsn_range: Range<Lsn>,
        _reconstruct_state: &mut ValueReconstructState,
    ) -> Result<ValueReconstructResult> {
        bail!(
            "layer {} needs to be downloaded",
            self.filename().file_name()
        );
    }

    fn is_incremental(&self) -> bool {
        self.is_incremental
    }

    /// debugging function to print out the contents of the layer
    fn dump(&self, _verbose: bool) -> Result<()> {
        println!(
            "----- remote layer for ten {} tli {} keys {}-{} lsn {}-{} ----",
            self.tenantid,
            self.timelineid,
            self.key_range.start,
            self.key_range.end,
            self.lsn_range.start,
            self.lsn_range.end
        );

        Ok(())
    }

    fn short_id(&self) -> String {
        self.filename().file_name()
    }
}

impl PersistentLayer for RemoteLayer {
    fn get_tenant_id(&self) -> TenantId {
        self.tenantid
    }

    fn get_timeline_id(&self) -> TimelineId {
        self.timelineid
    }

    fn filename(&self) -> LayerFileName {
        if self.is_delta {
            DeltaFileName {
                key_range: self.key_range.clone(),
                lsn_range: self.lsn_range.clone(),
            }
            .into()
        } else {
            ImageFileName {
                key_range: self.key_range.clone(),
                lsn: self.lsn_range.start,
            }
            .into()
        }
    }

    fn local_path(&self) -> Option<PathBuf> {
        None
    }

    fn iter(&self) -> Result<Box<dyn Iterator<Item = anyhow::Result<(Key, Lsn, Value)>> + '_>> {
        bail!("cannot iterate a remote layer");
    }

    fn key_iter(&self) -> Result<Box<dyn Iterator<Item = (Key, Lsn, u64)> + '_>> {
        bail!("cannot iterate a remote layer");
    }

    fn delete(&self) -> Result<()> {
        Ok(())
    }

    fn downcast_remote_layer<'a>(self: Arc<Self>) -> Option<std::sync::Arc<RemoteLayer>> {
        Some(self)
    }

    fn is_remote_layer(&self) -> bool {
        true
    }
}

impl RemoteLayer {
    pub fn new_img(
        tenantid: TenantId,
        timelineid: TimelineId,
        fname: &ImageFileName,
        layer_metadata: &LayerFileMetadata,
    ) -> RemoteLayer {
        RemoteLayer {
            tenantid,
            timelineid,
            key_range: fname.key_range.clone(),
            lsn_range: fname.lsn..(fname.lsn + 1),
            download_watch: Mutex::new(None),
            is_delta: false,
            is_incremental: false,
            file_name: fname.to_owned().into(),
            layer_metadata: layer_metadata.clone(),
        }
    }

    pub fn new_delta(
        tenantid: TenantId,
        timelineid: TimelineId,
        fname: &DeltaFileName,
        layer_metadata: &LayerFileMetadata,
    ) -> RemoteLayer {
        RemoteLayer {
            tenantid,
            timelineid,
            key_range: fname.key_range.clone(),
            lsn_range: fname.lsn_range.clone(),
            download_watch: Mutex::new(None),
            is_delta: true,
            is_incremental: true,
            file_name: fname.to_owned().into(),
            layer_metadata: layer_metadata.clone(),
        }
    }

    /// Create a Layer struct representing this layer, after it has been downloaded.
    pub fn create_downloaded_layer(
        &self,
        conf: &'static PageServerConf,
    ) -> Arc<dyn PersistentLayer> {
        if self.is_delta {
            let fname = DeltaFileName {
                key_range: self.key_range.clone(),
                lsn_range: self.lsn_range.clone(),
            };
            Arc::new(DeltaLayer::new(
                conf,
                self.timelineid,
                self.tenantid,
                &fname,
            ))
        } else {
            let fname = ImageFileName {
                key_range: self.key_range.clone(),
                lsn: self.lsn_range.start,
            };
            Arc::new(ImageLayer::new(
                conf,
                self.timelineid,
                self.tenantid,
                &fname,
            ))
        }
    }
}