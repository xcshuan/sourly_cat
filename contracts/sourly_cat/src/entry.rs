// Import from `core` instead of from `std` since we are in no-std mode
use core::result::Result;

// Import heap related library from `alloc`
// https://doc.rust-lang.org/alloc/index.html
use alloc::vec::Vec;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{load_cell_data, load_cell_lock_hash, load_script, load_witness_args, QueryIter},
};

use super::hash;
use crate::error::Error;

//hash 20 bytes + fishes 4 bytes + name (min 2bytes)
const MIN_NFT_DATA_LEN: usize = 40;
#[derive(PartialEq, Clone, Debug)]
pub struct NFTData {
    name: [u8; 16],
    hash: [u8; 20],
    fishes: i32,
}

impl From<&[u8]> for NFTData {
    fn from(data: &[u8]) -> Self {
        let mut name = [0u8; 16];
        name.copy_from_slice(&data[0..16]);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&data[16..36]);
        let mut fishes_byte = [0u8; 4];
        fishes_byte.copy_from_slice(&data[36..40]);
        //采用大端序
        let fishes = i32::from_be_bytes(fishes_byte);
        NFTData {
            name: name,
            hash: hash,
            fishes: fishes,
        }
    }
}

#[derive(Debug)]
pub struct Statistics {
    hp: u8,
    atk: u8,
    def: u8,
    lck: u8,
}

impl From<[u8; 20]> for Statistics {
    fn from(hash: [u8; 20]) -> Self {
        //只看每五个字节的最后一个字节
        let hp = hash[4] % 100 + 1;
        let atk = hash[9] % 100 + 1;
        let def = hash[14] % 100 + 1;
        let lck = hash[19] % 100 + 1;
        return Statistics { hp, atk, def, lck };
    }
}

// Owner具有创建NFT的权力
fn check_owner_mode(args: &Bytes) -> Result<bool, Error> {
    // With owner lock script extracted, we will look through each input in the
    // current transaction to see if any unlocked cell uses owner lock.
    let is_owner_mode = QueryIter::new(load_cell_lock_hash, Source::Input)
        .find(|lock_hash| args[..] == lock_hash[..])
        .is_some();
    Ok(is_owner_mode)
}

//将所有的输入NFT收集起来
fn collect_inputs_data() -> Result<Vec<NFTData>, Error> {
    let udt_list: Vec<NFTData> = QueryIter::new(load_cell_data, Source::GroupInput)
        .map(|data| {
            if data.len() == MIN_NFT_DATA_LEN {
                // u128 is 16 bytes
                Ok(NFTData::from(&data[..]))
            } else {
                Err(Error::Encoding)
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(udt_list)
}

//将所有的输出NFT收集起来
fn collect_outputs_data() -> Result<Vec<NFTData>, Error> {
    let udt_list: Vec<NFTData> = QueryIter::new(load_cell_data, Source::GroupOutput)
        .map(|data| {
            if data.len() >= MIN_NFT_DATA_LEN {
                // u128 is 16 bytes
                Ok(NFTData::from(&data[..]))
            } else {
                Err(Error::Encoding)
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(udt_list)
}

pub fn main() -> Result<(), Error> {
    let script = load_script()?;

    let args: Bytes = script.args().unpack();
    // debug!("script args is {:?}", args);

    // //Collect output NFT first
    let output_nft = collect_outputs_data()?;

    //Onwer发起的，Create NFT,
    if check_owner_mode(&args)? {
        //对Owner的操作不做检查
        return Ok(());
    }

    //收集所有的输入NFT
    let input_nft = collect_inputs_data()?;

    //输入和输出的数量必须相等
    if input_nft.len() != output_nft.len() {
        return Err(Error::ErrWrongInputOutPut);
    }

    if input_nft.len() == 2 {
        //从Witness中读取战斗轮次
        let mut n = 0;
        let res = load_witness_args(1, Source::GroupInput);
        if let Ok(wit_args) = res {
            let in_type = wit_args.input_type().as_bytes().to_vec();
            //in_type前四个字节好像被占用了
            if in_type.len() >= 6 {
                let mut number = [0u8; 2];
                number[0] = in_type[4];
                number[1] = in_type[5];
                n = u16::from_be_bytes(number);
            }
        }

        //Fighting
        if n > 0 {
            //战斗的时候，只允许有两个输入Cell即 0，1
            let res = load_cell_data(2, Source::Input);
            if !res.is_err() {
                return Err(Error::ErrWrongInputOutPut);
            }

            //其中一方不能再战斗了
            if input_nft[0].fishes < 0 || input_nft[1].fishes < 0 {
                return Err(Error::ErrUnmatchPlayer);
            }
            //名字不能变
            if input_nft[0].name != output_nft[0].name || input_nft[1].name != output_nft[1].name {
                return Err(Error::ErrWrongResult);
            }

            //要求挑战结束后，归属权不变
            let args_input1 = load_cell_lock_hash(0, Source::GroupInput)?;
            let args_input2 = load_cell_lock_hash(1, Source::GroupInput)?;
            let args_output1 = load_cell_lock_hash(0, Source::GroupOutput)?;
            let args_output2 = load_cell_lock_hash(1, Source::GroupOutput)?;

            //保证挑战双方一一对应
            if !args_input1.eq(&args_output1) || !args_input2.eq(&args_output2) {
                return Err(Error::ErrUnmatchPlayer);
            }

            //计算双方的挑战前属性值
            let stats_0: Statistics = (input_nft[0].hash).into();
            let stats_1: Statistics = (input_nft[1].hash).into();
            //debug!("stats_0:{:?},stats_1:{:?}", stats_0, stats_1);

            //计算攻击伤害
            // Hurt1 = ATK1*( 1 - DEF2/(DEF2 - LCK2*2 + 250) )
            let hurt_0 = stats_0.atk as u16
                - stats_0.atk as u16 * stats_1.def as u16
                    / (250 - stats_1.lck as u16 * 2 + stats_1.def as u16);

            // Hurt2 = ATK2*( 1 - DEF1/(DEF1 - LCK1*2 + 250) )
            let hurt_1 = stats_1.atk as u16
                - stats_1.atk as u16 * stats_0.def as u16
                    / (250 - stats_0.lck as u16 * 2 + stats_0.def as u16);

            //debug!("hurt_0:{},hurt_1:{}", hurt_0, hurt_1);

            //开始回合制攻击

            //0 noone win;1, 1 win;2, 2 win;3, all failed
            let mut who_win = 0;

            // 传入任意 n 值，满足下列两个条件之一，则可以确认战斗结果
            //n * Hurt1 > 10 * HP2 且 (n-1) * Hurt2 < 10 * HP1 则 <被挑战者> 胜利
            //n * Hurt1 < 10 * HP2 且 n * Hurt2 > 10 * HP1 则 <挑战者> 胜利
            if (n * hurt_0 >= 5 * stats_1.hp as u16) && ((n - 1) * (hurt_1) < 5 * stats_0.hp as u16)
            {
                who_win += 1;
            }

            //验证挑战结果
            if (n * hurt_0 < 5 * stats_1.hp as u16) && (n * (hurt_1) >= 5 * stats_0.hp as u16) {
                who_win += 2;
            }

            if who_win == 1 {
                //1 Win!
                //计算输的一方有多少fish，暂时没考虑四舍五入
                let mut loser_fishes = input_nft[1].fishes - stats_0.atk as i32 / 10;

                //触发隐藏奖励
                if loser_fishes == 0 {
                    loser_fishes = 999
                }

                //计算赢的一方的Fish数目
                let winner_fishes = { input_nft[0].fishes + (stats_1.hp as i32 / 10) };

                // debug!(
                //     "0 Win, loser_fishes:{}, winner_fishes:{}",
                //     loser_fishes, winner_fishes
                // );
                //检查fish是否对应
                if (output_nft[0].fishes != winner_fishes) || (output_nft[1].fishes != loser_fishes)
                {
                    return Err(Error::ErrWrongResult);
                }

                //输的一方要更改Hash, blake160(hash+lock_args)
                let lock_args = load_cell_lock_hash(0, Source::GroupInput)?;
                let mut conc = Vec::with_capacity(20 + lock_args.len());
                conc.extend(input_nft[1].hash.iter());
                conc.extend(lock_args.iter());
                let res = hash::blake2b_160(conc);

                //检验Hash是否相等，赢的一方不变
                if !res.eq(&output_nft[1].hash) || input_nft[0].hash != output_nft[0].hash {
                    return Err(Error::ErrWrongResult);
                }
                return Ok(());
            } else if who_win == 2 {
                //2 Win! 检查逻辑类似1
                let mut loser_fishes = input_nft[0].fishes - stats_1.atk as i32 / 10;
                if loser_fishes == 0 {
                    loser_fishes = 999
                }
                let winner_fishes = { input_nft[1].fishes + stats_0.hp as i32 / 10 };
                // debug!(
                //     "1 Win, loser_fishes:{}, winner_fishes:{}",
                //     loser_fishes, winner_fishes
                // );
                if (output_nft[1].fishes != winner_fishes) || (output_nft[0].fishes != loser_fishes)
                {
                    return Err(Error::ErrWrongResult);
                }
                let lock_args = load_cell_lock_hash(1, Source::GroupInput)?;
                let mut conc = Vec::with_capacity(20 + lock_args.len());
                conc.extend(input_nft[0].hash.iter());
                conc.extend(lock_args.iter());
                let res = hash::blake2b_160(conc);

                //检验Hash是否相等
                if !res.eq(&output_nft[0].hash) || input_nft[1].hash != output_nft[1].hash {
                    return Err(Error::ErrWrongResult);
                }

                return Ok(());
            }

            //没产生结果，报错
            return Err(Error::ErrWrongResult);
        }
    }

    //否则就是转账逻辑，一一检查是否相等
    if input_nft
        .into_iter()
        .zip(output_nft)
        .find(|(input, output)| {
            if *input != *output {
                return true;
            }
            return false;
        })
        .is_some()
    {
        return Err(Error::ErrWrongTransfer);
    }

    Ok(())
}
