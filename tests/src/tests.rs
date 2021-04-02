use super::*;
use blake2b_rs::Blake2bBuilder;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_tool::ckb_types::{bytes::Bytes, core::TransactionBuilder, packed::*, prelude::*};
use rand::{thread_rng, Rng};

const MAX_CYCLES: u64 = 10_000_000;

pub const CKB_HASH_PERSONALIZATION: &[u8] = b"ckb-default-hash";
pub const BLANK_HASH: [u8; 32] = [
    68, 244, 198, 151, 68, 213, 248, 197, 93, 100, 32, 98, 148, 157, 202, 228, 155, 196, 231, 239,
    67, 211, 136, 197, 161, 47, 66, 181, 99, 61, 22, 62,
];

pub fn new_blake2b() -> blake2b_rs::Blake2b {
    Blake2bBuilder::new(32)
        .personal(CKB_HASH_PERSONALIZATION)
        .build()
}

fn inner_blake2b_256<T: AsRef<[u8]>>(s: T) -> [u8; 32] {
    let mut result = [0u8; 32];
    let mut blake2b = new_blake2b();
    blake2b.update(s.as_ref());
    blake2b.finalize(&mut result);
    result
}

pub fn blake2b_256<T: AsRef<[u8]>>(s: T) -> [u8; 32] {
    if s.as_ref().is_empty() {
        return BLANK_HASH;
    }
    inner_blake2b_256(s)
}

pub fn blake2b_160<T: AsRef<[u8]>>(s: T) -> [u8; 20] {
    let mut result = [0u8; 20];
    let hash = blake2b_256(s);
    result.copy_from_slice(&hash[0..20]);
    result
}

#[derive(PartialEq, Clone, Debug)]
pub struct NFTData {
    name: [u8; 16],
    hash: [u8; 20],
    fishes: i32,
}

impl NFTData {
    fn new() -> Self {
        return NFTData {
            name: [0u8; 16],
            hash: [0u8; 20],
            fishes: 0,
        };
    }

    fn serialize(&self) -> [u8; 40] {
        let mut buf = [0u8; 40];
        buf[0..16].copy_from_slice(&self.name);
        buf[16..36].copy_from_slice(&self.hash);
        buf[36..40].copy_from_slice(&self.fishes.to_be_bytes());
        return buf;
    }
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

pub fn random_20bytes() -> Bytes {
    let mut rng = thread_rng();
    let mut buf = vec![0u8; 20];
    rng.fill(&mut buf[..]);
    Bytes::from(buf)
}

#[test]
fn test_create_nft() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("sourly_cat");
    let out_point = context.deploy_cell(contract_bin);
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    let lock_args = random_20bytes();
    // prepare scripts
    let lock_script = context
        .build_script(&always_success_out_point, lock_args)
        .expect("lock script");
    let lock_hash = lock_script.calc_script_hash().as_bytes();

    let lock_script_user = context
        .build_script(&always_success_out_point, random_20bytes())
        .expect("lock script");
    let lock_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point.clone())
        .build();
    // prepare scripts
    let type_script = context.build_script(&out_point, lock_hash).expect("script");
    let type_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(2000u64.pack())
            .lock(lock_script.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
    ];

    let mut nft_data = vec![NFTData::new(); 4];
    for u in &mut nft_data {
        u.fishes = 9;
    }
    let user_lock_hash = Vec::from(lock_script_user.calc_script_hash().as_slice());
    let mut conc: Vec<u8> = Vec::with_capacity(23);
    nft_data[0].name[0..4].copy_from_slice(b"125$");
    conc.extend(b"125".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[0].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[1].name[0..4].copy_from_slice(b"123$");
    conc.extend(b"123".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[1].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[2].name[0..4].copy_from_slice(b"234$");
    conc.extend(b"234".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[2].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[3].name[0..4].copy_from_slice(b"456$");
    conc.extend(b"456".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[3].hash = blake2b_160(&conc);
    conc.clear();

    let mut outputs_data = Vec::with_capacity(4);
    for nft in nft_data.into_iter() {
        let data = nft.serialize().to_vec();
        outputs_data.push(Bytes::from(data));
    }
    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(type_script_dep)
        .cell_dep(lock_script_dep)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_nft_transfer() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("sourly_cat");
    let out_point = context.deploy_cell(contract_bin);
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    let lock_args = random_20bytes();
    // prepare scripts
    let lock_script = context
        .build_script(&always_success_out_point, lock_args)
        .expect("lock script");
    let lock_hash = lock_script.calc_script_hash().as_bytes();
    let lock_script_input = context
        .build_script(&always_success_out_point, random_20bytes())
        .expect("lock script");
    let lock_script_user = context
        .build_script(&always_success_out_point, random_20bytes())
        .expect("lock script");
    let lock_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point.clone())
        .build();

    // prepare scripts
    let type_script = context.build_script(&out_point, lock_hash).expect("script");
    let type_script_dep = CellDep::new_builder().out_point(out_point).build();

    let mut nft_data = vec![NFTData::new(); 4];
    for u in &mut nft_data {
        u.fishes = 9;
    }
    let user_lock_hash = Vec::from(lock_script_user.calc_script_hash().as_slice());
    let mut conc: Vec<u8> = Vec::with_capacity(23);
    nft_data[0].name[0..4].copy_from_slice(b"125$");
    conc.extend(b"125".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[0].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[1].name[0..4].copy_from_slice(b"123$");
    conc.extend(b"123".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[1].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[2].name[0..4].copy_from_slice(b"234$");
    conc.extend(b"234".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[2].hash = blake2b_160(&conc);
    conc.clear();

    nft_data[3].name[0..4].copy_from_slice(b"456$");
    conc.extend(b"456".iter());
    conc.extend(user_lock_hash.iter());
    nft_data[3].hash = blake2b_160(&conc);
    conc.clear();

    let mut outputs_data = Vec::with_capacity(4);
    for nft in nft_data.into_iter() {
        let data = nft.serialize().to_vec();
        outputs_data.push(Bytes::from(data));
    }

    // prepare cells
    let input_out_points = [
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            outputs_data[0].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            outputs_data[1].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            outputs_data[2].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            outputs_data[3].clone(),
        ),
    ];
    let inputs = vec![
        CellInput::new_builder()
            .previous_output(input_out_points[0].clone())
            .build(),
        CellInput::new_builder()
            .previous_output(input_out_points[1].clone())
            .build(),
        CellInput::new_builder()
            .previous_output(input_out_points[2].clone())
            .build(),
        CellInput::new_builder()
            .previous_output(input_out_points[3].clone())
            .build(),
    ];
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_user.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
    ];

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(type_script_dep)
        .cell_dep(lock_script_dep)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_nft_fighting() {
    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("sourly_cat");
    let out_point = context.deploy_cell(contract_bin);
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    let lock_args = random_20bytes();
    // prepare scripts
    let lock_script = context
        .build_script(&always_success_out_point, lock_args)
        .expect("lock script");
    let lock_hash = lock_script.calc_script_hash().as_bytes();
    // prepare scripts
    let lock_script_1 = context
        .build_script(&always_success_out_point, random_20bytes())
        .expect("lock script");
    let lock_script_2 = context
        .build_script(&always_success_out_point, random_20bytes())
        .expect("lock script");
    let lock_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point.clone())
        .build();
    // prepare scripts
    let type_script = context.build_script(&out_point, lock_hash).expect("script");
    let type_script_dep = CellDep::new_builder().out_point(out_point).build();

    let mut input_nft = vec![NFTData::new(); 2];
    for nft in &mut input_nft {
        nft.fishes = 9;
    }
    let lock_hash_1 = Vec::from(lock_script_1.calc_script_hash().as_slice());
    let lock_hash_2 = Vec::from(lock_script_2.calc_script_hash().as_slice());
    //第一个NFT
    let mut conc: Vec<u8> = Vec::with_capacity(23);
    //名字125
    input_nft[0].name[0..4].copy_from_slice(b"125$");
    conc.extend(b"125".iter());
    conc.extend(lock_hash_1.iter());
    input_nft[0].hash = blake2b_160(&conc);
    conc.clear();

    //第二个NFT
    //名字123
    input_nft[1].name[0..4].copy_from_slice(b"123$");
    conc.extend(b"123".iter());
    conc.extend(lock_hash_2.iter());
    input_nft[1].hash = blake2b_160(&conc);
    conc.clear();

    let mut inputs_data = Vec::with_capacity(2);
    for nft in input_nft.iter() {
        let data = nft.serialize().to_vec();
        inputs_data.push(Bytes::from(data));
    }
    // prepare cells
    let input_out_points = [
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_1.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            inputs_data[0].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_2.clone())
                .type_(
                    ScriptOpt::new_builder()
                        .set(Some(type_script.clone()))
                        .build(),
                )
                .build(),
            inputs_data[1].clone(),
        ),
    ];

    let inputs = vec![
        CellInput::new_builder()
            .previous_output(input_out_points[0].clone())
            .build(),
        CellInput::new_builder()
            .previous_output(input_out_points[1].clone())
            .build(),
    ];

    let max_fight_count: u16 = 2000;

    let mut output_nft = vec![NFTData::new(); 2];

    output_nft[0].name = input_nft[0].name;
    output_nft[1].name = input_nft[1].name;

    //计算双方的挑战前属性值
    let stats_1: Statistics = (input_nft[0].hash).into();
    let stats_2: Statistics = (input_nft[1].hash).into();

    // print!("stats_1:{:?},stats_2:{:?}\n", stats_1, stats_2);

    //计算攻击伤害
    // Hurt1 = ATK1*( 1 - DEF2/(DEF2 - LCK2*2 + 250) )
    let hurt_1 = stats_1.atk as u16
        * (1 - stats_2.def as u16 / (250 - stats_2.lck as u16 * 2 + stats_2.def as u16));

    // Hurt2 = ATK2*( 1 - DEF1/(DEF1 - LCK1*2 + 250) )
    let hurt_2 = stats_2.atk as u16
        * (1 - stats_1.def as u16 / (250 - stats_1.lck as u16 * 2 + stats_1.def as u16));

    // print!("hurt_1:{},hurt_2:{}\n", hurt_1, hurt_2);

    let mut someone_win = false;
    let mut n = 0;
    for i in 1..=max_fight_count {
        if (i as u16 * hurt_1 > 10 * stats_2.hp as u16)
            && ((i - 1) as u16 * (hurt_2) < 10 * stats_1.hp as u16)
        {
            n = i;
            someone_win = true;
            //1 Win!

            //计算输的一方有多少fish，暂时没考虑四舍五入
            let mut loser_fishes = input_nft[1].fishes - stats_1.atk as i32 / 10;

            //触发隐藏奖励
            if loser_fishes == 0 {
                loser_fishes = 999
            }

            //计算赢的一方的Fish数目
            let winner_fishes = { input_nft[0].fishes + (stats_2.hp as i32 / 10) };

            print!(
                "1Win, loser_fishes:{}, winner_fishes:{}\n",
                loser_fishes, winner_fishes
            );

            output_nft[0].fishes = winner_fishes;
            output_nft[1].fishes = loser_fishes;

            //输的一方要更改Hash, blake160(hash+lock_hash)
            let lock_hash = Vec::from(lock_script_1.calc_script_hash().as_slice());
            let mut conc = Vec::with_capacity(20 + lock_hash.len());
            conc.extend(input_nft[1].hash.iter());
            conc.extend(lock_hash.iter());
            let res = blake2b_160(conc);

            output_nft[1].hash = res;
            output_nft[0].hash = input_nft[0].hash;
            break;
        }

        //验证挑战结果
        if (i as u16 * hurt_1 < 10 * stats_2.hp as u16)
            && (i as u16 * (hurt_2) > 10 * stats_1.hp as u16)
        {
            n = i;
            someone_win = true;
            //2 Win! 检查逻辑类似1
            let mut loser_fishes = input_nft[0].fishes - stats_2.atk as i32 / 10;
            if loser_fishes == 0 {
                loser_fishes = 999
            }
            let winner_fishes = { input_nft[1].fishes + stats_1.hp as i32 / 10 };

            print!(
                "2Win, loser_fishes:{}, winner_fishes:{}\n",
                loser_fishes, winner_fishes
            );
            output_nft[1].fishes = winner_fishes;
            output_nft[0].fishes = loser_fishes;

            let lock_hash = Vec::from(lock_script_2.calc_script_hash().as_slice());

            let mut conc = Vec::with_capacity(20 + lock_hash.len());
            conc.extend(input_nft[0].hash.iter());
            conc.extend(lock_hash.iter());
            let res = blake2b_160(conc);

            output_nft[0].hash = res;
            output_nft[1].hash = input_nft[1].hash;
            break;
        }
    }

    if !someone_win {
        panic!("Even!")
    }
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_1.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script_2.clone())
            .type_(
                ScriptOpt::new_builder()
                    .set(Some(type_script.clone()))
                    .build(),
            )
            .build(),
    ];

    let mut outputs_data = Vec::with_capacity(4);
    for nft in output_nft.into_iter() {
        let data = nft.serialize().to_vec();
        outputs_data.push(Bytes::from(data));
    }
    let fight_number = n.to_be_bytes().to_vec();
    print!("n = {}, fight_number = {:?}\n", n, fight_number);
    let witnesses = vec![
        WitnessArgsBuilder::default().build().as_bytes().pack(),
        WitnessArgsBuilder::default()
            .input_type(
                BytesOpt::new_builder()
                    .set(Some(Bytes::from(fight_number).pack()))
                    .build(),
            )
            .build()
            .as_bytes()
            .pack(),
    ];
    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .set_witnesses(witnesses)
        .outputs_data(outputs_data.pack())
        .cell_dep(type_script_dep)
        .cell_dep(lock_script_dep)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_fighting_prob() {
    // deploy contract
    let mut context = Context::default();

    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

    let mut win_1_count = 0;
    let mut win_2_count = 0;
    let mut even_count = 0;

    for k in 1..=10 as u16 {
        let all = 100000;
        for _ in 0..all {
            // prepare scripts
            let lock_script_1 = context
                .build_script(&always_success_out_point, random_20bytes())
                .expect("lock script");
            let lock_script_2 = context
                .build_script(&always_success_out_point, random_20bytes())
                .expect("lock script");
            let mut input_nft = vec![NFTData::new(); 2];
            for nft in &mut input_nft {
                nft.fishes = 9;
            }
            let lock_hash_1 = Vec::from(lock_script_1.calc_script_hash().as_slice());
            let lock_hash_2 = Vec::from(lock_script_2.calc_script_hash().as_slice());
            //第一个NFT
            let mut conc: Vec<u8> = Vec::with_capacity(23);
            //名字125
            input_nft[0].name[0..4].copy_from_slice(b"125$");
            conc.extend(b"125".iter());
            conc.extend(lock_hash_1.iter());
            input_nft[0].hash = blake2b_160(&conc);
            conc.clear();

            //第二个NFT
            //名字123
            input_nft[1].name[0..4].copy_from_slice(b"123$");
            conc.extend(b"123".iter());
            conc.extend(lock_hash_2.iter());
            input_nft[1].hash = blake2b_160(&conc);
            conc.clear();

            let mut inputs_data = Vec::with_capacity(2);
            for nft in input_nft.iter() {
                let data = nft.serialize().to_vec();
                inputs_data.push(Bytes::from(data));
            }

            let mut output_nft = vec![NFTData::new(); 2];

            output_nft[0].name = input_nft[0].name;
            output_nft[1].name = input_nft[1].name;

            //计算双方的挑战前属性值
            let stats_1: Statistics = (input_nft[0].hash).into();
            let stats_2: Statistics = (input_nft[1].hash).into();

            // print!("n:{},stats_1:{:?},stats_2:{:?}\n", n, stats_1, stats_2);

            //计算攻击伤害
            // Hurt1 = ATK1*( 1 - DEF2/(DEF2 - LCK2*2 + 250) )
            let hurt_1 = stats_1.atk as u16
                * (1 - stats_2.def as u16 / (250 - stats_2.lck as u16 * 2 + stats_2.def as u16));

            // Hurt2 = ATK2*( 1 - DEF1/(DEF1 - LCK1*2 + 250) )
            let hurt_2 = stats_2.atk as u16
                * (1 - stats_1.def as u16 / (250 - stats_1.lck as u16 * 2 + stats_1.def as u16));

            // print!("hurt_1:{},hurt_2:{}\n", hurt_1, hurt_2);

            let mut someone_win = false;
            for i in 1..5000 as u16 {
                if (i * hurt_1) >= (k * stats_2.hp as u16)
                    && ((i - 1) * hurt_2) < (k * stats_1.hp as u16)
                {
                    someone_win = true;
                    win_1_count += 1;
                    //1 Win!

                    //计算输的一方有多少fish，暂时没考虑四舍五入
                    let mut loser_fishes = input_nft[1].fishes - stats_1.atk as i32 / 10;

                    //触发隐藏奖励
                    if loser_fishes == 0 {
                        loser_fishes = 999
                    }

                    //计算赢的一方的Fish数目
                    let winner_fishes = { input_nft[0].fishes + (stats_2.hp as i32 / 10) };

                    // print!(
                    //     "1Win, loser_fishes:{}, winner_fishes:{}\n",
                    //     loser_fishes, winner_fishes
                    // );

                    output_nft[0].fishes = winner_fishes;
                    output_nft[1].fishes = loser_fishes;

                    //输的一方要更改Hash, blake160(hash+lock_hash)
                    let lock_hash = Vec::from(lock_script_1.calc_script_hash().as_slice());
                    let mut conc = Vec::with_capacity(20 + lock_hash.len());
                    conc.extend(input_nft[1].hash.iter());
                    conc.extend(lock_hash.iter());
                    let res = blake2b_160(conc);

                    output_nft[1].hash = res;
                    output_nft[0].hash = input_nft[0].hash;
                    break;
                }

                //验证挑战结果
                if ((i * hurt_1) < (k * stats_2.hp as u16))
                    && (i * hurt_2) >= (k * stats_1.hp as u16)
                {
                    someone_win = true;
                    win_2_count += 1;
                    //2 Win! 检查逻辑类似1
                    let mut loser_fishes = input_nft[0].fishes - stats_2.atk as i32 / 10;
                    if loser_fishes == 0 {
                        loser_fishes = 999
                    }
                    let winner_fishes = { input_nft[1].fishes + stats_1.hp as i32 / 10 };

                    // print!(
                    //     "2Win, loser_fishes:{}, winner_fishes:{}\n",
                    //     loser_fishes, winner_fishes
                    // );
                    output_nft[1].fishes = winner_fishes;
                    output_nft[0].fishes = loser_fishes;

                    let lock_hash = Vec::from(lock_script_2.calc_script_hash().as_slice());

                    let mut conc = Vec::with_capacity(20 + lock_hash.len());
                    conc.extend(input_nft[0].hash.iter());
                    conc.extend(lock_hash.iter());
                    let res = blake2b_160(conc);

                    output_nft[0].hash = res;
                    output_nft[1].hash = input_nft[1].hash;
                    break;
                }
            }
            if !someone_win {
                even_count += 1
            }
        }
        print!(
            "k is {}, 1 Win count:{}, 2 Win count:{}, Even count:{}\n",
            k,
            win_1_count as f64 / all as f64,
            win_2_count as f64 / all as f64,
            even_count as f64 / all as f64
        );
        win_1_count = 0;
        win_2_count = 0;
        even_count = 0;
    }
}
