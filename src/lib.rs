pub mod config;
pub mod cube;
pub mod state_machine;

use uuid::{Uuid, uuid};

pub const GAN_GEN2_SERVICE: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dc4179");
pub const GAN_GEN3_SERVICE: Uuid = uuid!("8653000a-43e6-47b7-9cb0-5fc21d4ae340");
pub const GAN_GEN4_SERVICE: Uuid = uuid!("00000010-0000-fff7-fff6-fff5fff4fff0");
