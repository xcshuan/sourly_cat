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
    debug,
    high_level::{load_cell_data, load_cell_lock_hash, load_script, load_witness_args, QueryIter},
};

use super::hash;
use crate::error::Error;

//hash 20 bytes + fishes 4 bytes + name (min 2bytes)
const MIN_NFT_DATA_LEN: usize = 40;

#[derive(PartialEq)]
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

pub struct Statistics {
    hp: u8,
    atk: u8,
    def: u8,
    lck: u8,
}

impl From<[u8; 20]> for Statistics {
    fn from(hash: [u8; 20]) -> Self {
        //只看每五个字节的最后一个字节
        let hp = hash[4] % 101;
        let atk = hash[9] % 101;
        let def = hash[14] % 101;
        let lck = hash[19] % 101;
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
    debug!("script args is {:?}", args);

    //Collect output NFT first
    let output_nft = collect_outputs_data()?;

    //Onwer发起的，Create NFT,
    if check_owner_mode(&args)? {
        //对每个生成的NFT，验证其是否符合规则
        output_nft
            .iter()
            .enumerate()
            .map(|(i, nft)| {
                if nft.fishes != 9 {
                    return Err(Error::ErrInvalidParas);
                }
                let lock_args = load_cell_lock_hash(i, Source::Output)?;
                let mut index = 16;
                //以空格前的字符作为name
                nft.name.iter().enumerate().any(|(i, v)| {
                    if *v == b' ' {
                        index = i + 1;
                        return true;
                    }
                    return false;
                });
                //最少两个字符
                if index < 3 {
                    return Err(Error::ErrInvalidParas);
                }
                //拼接同时求hash
                let mut conc = Vec::with_capacity(index + lock_args.len());
                conc[0..index].copy_from_slice(&nft.name[0..index]);
                conc[index..].copy_from_slice(&lock_args);
                let res = hash::blake2b_160(conc);

                //检验Hash是否相等
                if !res.eq(&nft.hash) {
                    return Err(Error::ErrWrongResult);
                }
                return Ok(());
            })
            .collect::<Result<Vec<_>, Error>>()?;
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
        let wit_args = load_witness_args(1, Source::Input)?;
        let in_type = wit_args.input_type().as_bytes().to_vec();
        if in_type.len() > 0 {
            n = in_type[0]
        }

        //Fighting
        if n > 0 {
            //其中一方不能再战斗了
            if input_nft[0].fishes < 0 || input_nft[1].fishes < 0 {
                return Err(Error::ErrUnmatchPlayer);
            }
            //名字不能变
            if input_nft[0].name != output_nft[0].name || input_nft[1].name != output_nft[1].name {
                return Err(Error::ErrWrongResult);
            }

            //要求挑战结束后，归属权不变
            let args_input1 = load_cell_lock_hash(0, Source::Input)?;
            let args_input2 = load_cell_lock_hash(1, Source::Input)?;
            let args_output1 = load_cell_lock_hash(0, Source::Output)?;
            let args_output2 = load_cell_lock_hash(1, Source::Output)?;

            //保证挑战双方一一对应
            if !(args_input1.eq(&args_output1) || args_input2.eq(&args_output2)) {
                return Err(Error::ErrUnmatchPlayer);
            }

            //计算双方的挑战前属性值
            let stats_1: Statistics = (input_nft[0].hash).into();
            let stats_2: Statistics = (output_nft[0].hash).into();

            //计算攻击伤害
            // Hurt1 = ATK1*( 1 - DEF2/(DEF2 - LCK2*2 + 250) )
            let hurt_1 = stats_1.atk * (1 - stats_2.def / (stats_2.def - stats_2.lck * 2 + 250));

            // Hurt2 = ATK2*( 1 - DEF1/(DEF1 - LCK1*2 + 250) )
            let hurt_2 = stats_2.atk * (1 - stats_1.def / (stats_1.def - stats_1.lck * 2 + 250));

            //验证挑战结果
            if (n * hurt_1 > 10 * stats_2.hp) & ((n - 1) * hurt_2 < 10 * stats_1.hp) {
                //1 Win!

                //计算输的一方有多少fish，暂时没考虑四舍五入
                let mut loser_fishes = input_nft[1].fishes - stats_1.atk as i32 / 10;

                //触发隐藏奖励
                if loser_fishes == 0 {
                    loser_fishes = 999
                }

                //计算赢的一方的Fish数目
                let winner_fishes = { input_nft[0].fishes + (stats_2.hp as i32 / 10) };

                //检查fish是否对应
                if (output_nft[0].fishes != winner_fishes) || (output_nft[1].fishes != loser_fishes)
                {
                    return Err(Error::ErrWrongResult);
                }

                //输的一方要更改Hash
                let lock_args = load_cell_lock_hash(0, Source::Input)?;
                let mut conc = Vec::with_capacity(20 + lock_args.len());
                conc[0..20].copy_from_slice(&input_nft[1].hash);
                conc[20..].copy_from_slice(&lock_args);
                let res = hash::blake2b_160(conc);

                //检验Hash是否相等，赢的一方不变
                if !res.eq(&output_nft[1].hash) || input_nft[0].hash != output_nft[0].hash {
                    return Err(Error::ErrWrongResult);
                }
                return Ok(());
            } else {
                //2 Win! 检查逻辑类似1
                let mut loser_fishes = input_nft[0].fishes - stats_2.atk as i32 / 10;
                if loser_fishes == 0 {
                    loser_fishes = 999
                }
                let winner_fishes = { input_nft[1].fishes + stats_1.hp as i32 / 10 };
                if (output_nft[1].fishes != winner_fishes) || (output_nft[0].fishes != loser_fishes)
                {
                    return Err(Error::ErrWrongResult);
                }

                let lock_args = load_cell_lock_hash(1, Source::Input)?;
                let mut conc = Vec::with_capacity(20 + lock_args.len());
                conc[0..20].copy_from_slice(&input_nft[0].hash);
                conc[20..].copy_from_slice(&lock_args);
                let res = hash::blake2b_160(conc);

                //检验Hash是否相等
                if !res.eq(&output_nft[0].hash) || input_nft[1].hash != output_nft[1].hash {
                    return Err(Error::ErrWrongResult);
                }
            }
            return Ok(());
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
        return Err(Error::ErrWrongResult);
    }

    Ok(())
}