use aes::{
    Aes128, Block,
    cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray},
};
use anyhow::bail;
use btleplug::api::{Characteristic, Peripheral};
use futures::StreamExt;
use serde::Deserialize;

#[derive(Clone)]
struct GANCubeVersion2Cipher {
    device_key: [u8; 16],
    device_iv: [u8; 16],
}

impl GANCubeVersion2Cipher {
    fn decrypt(&self, value: &[u8]) -> anyhow::Result<Vec<u8>> {
        if value.len() <= 16 {
            bail!("Packet size less than expected length");
        }

        // Packets are larger than block size. First decrypt the last 16 bytes
        // of the packet in place.
        let mut value = value.to_vec();
        let aes = Aes128::new(GenericArray::from_slice(&self.device_key));
        let offset = value.len() - 16;
        let end_cipher = &value[offset..];
        let mut end_plain = Block::clone_from_slice(end_cipher);
        aes.decrypt_block(&mut end_plain);
        for i in 0..16 {
            end_plain[i] ^= self.device_iv[i];
            value[offset + i] = end_plain[i];
        }

        // Decrypt the first 16 bytes of the packet in place. This will overlap
        // with the decrypted block above.
        let start_cipher = &value[0..16];
        let mut start_plain = Block::clone_from_slice(start_cipher);
        aes.decrypt_block(&mut start_plain);
        for i in 0..16 {
            start_plain[i] ^= self.device_iv[i];
            value[i] = start_plain[i];
        }

        Ok(value)
    }

    fn encrypt(&self, value: &[u8]) -> anyhow::Result<Vec<u8>> {
        if value.len() <= 16 {
            bail!("Packet size less than expected length");
        }

        // Packets are larger than block size. First encrypt the first 16 bytes
        // of the packet in place.
        let mut value = value.to_vec();
        for i in 0..16 {
            value[i] ^= self.device_iv[i];
        }
        let mut cipher = Block::clone_from_slice(&value[0..16]);
        let aes = Aes128::new(GenericArray::from_slice(&self.device_key));
        aes.encrypt_block(&mut cipher);
        for i in 0..16 {
            value[i] = cipher[i];
        }

        // Decrypt the last 16 bytes of the packet in place. This will overlap
        // with the decrypted block above.
        let offset = value.len() - 16;
        for i in 0..16 {
            value[offset + i] ^= self.device_iv[i];
        }
        let mut cipher = Block::clone_from_slice(&value[offset..]);
        aes.encrypt_block(&mut cipher);
        for i in 0..16 {
            value[offset + i] = cipher[i];
        }

        Ok(value)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Deserialize)]
pub enum Move {
    U,
    Up,
    R,
    Rp,
    F,
    Fp,
    D,
    Dp,
    L,
    Lp,
    B,
    Bp,
}

const CUBE_MOVE_MESSAGE: u8 = 2;
const CUBE_STATE_MESSAGE: u8 = 4;
const CUBE_BATTERY_STATE_MESSAGE: u8 = 9;

pub async fn move_stream_v2(
    device: impl Peripheral,
    read: Characteristic,
    write: Characteristic,
) -> anyhow::Result<tokio::sync::broadcast::Receiver<Move>> {
    let device_key: [u8; 6] = if let Some(data) = device
        .properties()
        .await?
        .ok_or(anyhow::anyhow!("could not get device properties"))?
        .manufacturer_data
        .get(&36097)
    {
        if data.len() >= 9 {
            let mut result = [0; 6];
            result.copy_from_slice(&data[3..9]);
            result
        } else {
            bail!("Device identifier invalid")
        }
    } else {
        bail!("Manufacturer data missing device identifier")
    };

    const GAN_V2_KEY: [u8; 16] = [
        0x01, 0x02, 0x42, 0x28, 0x31, 0x91, 0x16, 0x07, 0x20, 0x05, 0x18, 0x54, 0x42, 0x11, 0x12,
        0x53,
    ];
    const GAN_V2_IV: [u8; 16] = [
        0x11, 0x03, 0x32, 0x28, 0x21, 0x01, 0x76, 0x27, 0x20, 0x95, 0x78, 0x14, 0x32, 0x12, 0x02,
        0x43,
    ];

    let mut key = GAN_V2_KEY.clone();
    let mut iv = GAN_V2_IV.clone();
    for (idx, byte) in device_key.iter().enumerate() {
        key[idx] = ((key[idx] as u16 + *byte as u16) % 255) as u8;
        iv[idx] = ((iv[idx] as u16 + *byte as u16) % 255) as u8;
    }

    let cipher = GANCubeVersion2Cipher {
        device_key: key,
        device_iv: iv,
    };

    let mut notificaitons = device.notifications().await?;

    let (tx, rx) = tokio::sync::broadcast::channel::<Move>(10);

    tokio::spawn(async move {
        let mut last_move_count = None;
        while let Some(value) = notificaitons.next().await {
            if let Ok(value) = cipher.decrypt(&value.value) {
                let message_type = extract_bits(&value, 0, 4) as u8;

                match message_type {
                    CUBE_MOVE_MESSAGE => {
                        let current_move_count = extract_bits(&value, 4, 8) as u8;

                        let Some(last) = last_move_count.as_mut() else {
                            last_move_count = Some(current_move_count);
                            continue;
                        };

                        let move_count = current_move_count - *last;
                        *last = current_move_count;

                        for j in 0..(move_count as usize) {
                            let i = (move_count as usize - 1) - j;

                            let move_num = extract_bits(&value, 12 + i * 5, 5) as usize;
                            const MOVES: &[Move] = &[
                                Move::U,
                                Move::Up,
                                Move::R,
                                Move::Rp,
                                Move::F,
                                Move::Fp,
                                Move::D,
                                Move::Dp,
                                Move::L,
                                Move::Lp,
                                Move::B,
                                Move::Bp,
                            ];

                            if move_num >= MOVES.len() {
                                continue;
                            }

                            tx.send(MOVES[move_num]).expect("could not broadcast move");
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    device.subscribe(&read).await?;

    Ok(rx)
}

fn extract_bits(data: &[u8], start: usize, count: usize) -> u32 {
    let mut result = 0;
    for i in 0..count {
        let bit = start + i;
        result <<= 1;
        if data[bit / 8] & (1 << (7 - (bit % 8))) != 0 {
            result |= 1;
        }
    }
    result
}
