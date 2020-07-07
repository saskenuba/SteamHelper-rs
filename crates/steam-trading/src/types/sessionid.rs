use erased_serde::serialize_trait_object;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionID {
    pub sessionid: String,
}

pub(crate) trait HasSessionID: erased_serde::Serialize {
    fn set_sessionid(&mut self, sessionid: String);
}

serialize_trait_object!(HasSessionID);
