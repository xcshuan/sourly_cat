use super::{hash::*, *};
use ckb_crypto::secp::Generator;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_tool::ckb_types::{
    bytes::Bytes,
    core::{TransactionBuilder, TransactionView},
    packed::*,
    prelude::*,
};

fn gen_tx_for_nft_fighting(context: &mut Context, lock_args: Bytes) -> TransactionView {
    let sourly_cat_out_point = context.deploy_cell(SOURLY_CAT_BIN.clone());
    let sighash_all_out_point = context.deploy_cell(KECCAK256_ALL_ACPL_BIN.clone());
    let secp_out_point = context.deploy_cell(SECP256K1_DATA_BIN.clone());
    let secp_dep = CellDep::new_builder()
        .out_point(secp_out_point.clone())
        .build();
    let sighash_all_dep = CellDep::new_builder()
        .out_point(sighash_all_out_point.clone())
        .build();

    let sourly_cat_dep = CellDep::new_builder()
        .out_point(sourly_cat_out_point.clone())
        .build();

    // prepare scripts
    let lock_script_1 = context
        .build_script(&sighash_all_out_point, random_20bytes())
        .expect("lock script");
    //挑战方签名
    let lock_script_2 = context
        .build_script(&sighash_all_out_point, lock_args)
        .expect("lock script");
    // prepare scripts
    let type_script = context
        .build_script(&&sourly_cat_out_point, random_20bytes())
        .expect("script");

    let lock_hash_1 = Vec::from(lock_script_1.calc_script_hash().as_slice());
    let lock_hash_2 = Vec::from(lock_script_2.calc_script_hash().as_slice());

    let input_nft = vec![
        NFTData::gen_random_nft(&lock_hash_1),
        NFTData::gen_random_nft(&lock_hash_2),
    ];

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

    let max_fight_count: u16 = 3000;

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
        if (i as u16 * hurt_1 > 5 * stats_2.hp as u16)
            && ((i - 1) as u16 * (hurt_2) < 5 * stats_1.hp as u16)
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
                "1 Win, loser_fishes:{}, winner_fishes:{}\n",
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
        if (i as u16 * hurt_1 < 5 * stats_2.hp as u16)
            && (i as u16 * (hurt_2) > 5 * stats_1.hp as u16)
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
                "2 Win, loser_fishes:{}, winner_fishes:{}\n",
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
        Bytes::new(),
        WitnessArgsBuilder::default()
            .lock(Some(Bytes::new()).pack())
            .input_type(Some(Bytes::from(fight_number)).pack())
            .build()
            .as_bytes(),
    ];
    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(sourly_cat_dep)
        .cell_dep(sighash_all_dep)
        .cell_dep(secp_dep)
        .witnesses(witnesses.pack())
        .build();
    let tx = context.complete_tx(tx);
    tx
}

#[test]
fn test_nft_fighting() {
    // deploy contract
    let mut context = Context::default();
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash =
        if get_current_chain_id() == CHAIN_ID_BTC || get_current_chain_id() == CHAIN_ID_DOGE {
            let pubkey = if is_compressed() {
                pubkey_compressed(&pubkey)
            } else {
                pubkey_uncompressed(&pubkey)
            };
            ripemd_sha(&pubkey)
        } else {
            eth160(pubkey)
        };

    let tx = gen_tx_for_nft_fighting(&mut context, pubkey_hash);

    //让第二个人签名
    let tx = sign_tx_by_input_group_keccak256(&mut context, tx, &privkey, 1, 1);
    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_fighting_prob() {
    if false {
        // deploy contract
        let mut context = Context::default();

        let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

        let mut win_1_count = 0;
        let mut win_2_count = 0;
        let mut even_count = 0;

        for k in 1..=10 as u16 {
            let all = 2000;
            for _ in 0..all {
                // prepare scripts
                let lock_script_1 = context
                    .build_script(&always_success_out_point, random_20bytes())
                    .expect("lock script");
                let lock_script_2 = context
                    .build_script(&always_success_out_point, random_20bytes())
                    .expect("lock script");
                let lock_hash_1 = Vec::from(lock_script_1.calc_script_hash().as_slice());
                let lock_hash_2 = Vec::from(lock_script_2.calc_script_hash().as_slice());
                let input_nft = vec![
                    NFTData::gen_random_nft(&lock_hash_1),
                    NFTData::gen_random_nft(&lock_hash_2),
                ];

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
                    * (1 - stats_2.def as u16
                        / (250 - stats_2.lck as u16 * 2 + stats_2.def as u16));

                // Hurt2 = ATK2*( 1 - DEF1/(DEF1 - LCK1*2 + 250) )
                let hurt_2 = stats_2.atk as u16
                    * (1 - stats_1.def as u16
                        / (250 - stats_1.lck as u16 * 2 + stats_1.def as u16));

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
}
