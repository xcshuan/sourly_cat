use super::{hash::*, *};
use ckb_crypto::secp::Generator;
use ckb_testtool::context::Context;
use ckb_tool::ckb_types::{
    bytes::Bytes,
    core::{TransactionBuilder, TransactionView},
    packed::*,
    prelude::*,
};

fn gen_tx_for_nft_transfer(context: &mut Context, lock_args: Bytes) -> TransactionView {
    //load_script_bin
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
    let lock_script_input = context
        .build_script(&sighash_all_out_point, lock_args)
        .expect("lock script");
    let lock_script_user = context
        .build_script(&sighash_all_out_point, random_20bytes())
        .expect("lock script");
    let type_script = context
        .build_script(&&sourly_cat_out_point, random_20bytes())
        .expect("script");

    let user_lock_hash = Vec::from(lock_script_user.calc_script_hash().as_slice());
    let input_nft = vec![
        NFTData::gen_random_nft(&user_lock_hash),
        NFTData::gen_random_nft(&user_lock_hash),
        NFTData::gen_random_nft(&user_lock_hash),
        NFTData::gen_random_nft(&user_lock_hash),
    ];

    let mut cell_data = Vec::with_capacity(4);
    for nft in input_nft.into_iter() {
        let data = nft.serialize().to_vec();
        cell_data.push(Bytes::from(data));
    }

    // prepare cells
    let input_out_points = [
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(Some(type_script.clone()).pack())
                .build(),
            cell_data[0].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(Some(type_script.clone()).pack())
                .build(),
            cell_data[1].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(Some(type_script.clone()).pack())
                .build(),
            cell_data[2].clone(),
        ),
        context.create_cell(
            CellOutput::new_builder()
                .capacity(500u64.pack())
                .lock(lock_script_input.clone())
                .type_(Some(type_script.clone()).pack())
                .build(),
            cell_data[3].clone(),
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

    let mut witnesses = vec![];
    witnesses.push(WitnessArgsBuilder::default().build().as_bytes());
    for _ in 1..inputs.len() {
        witnesses.push(Bytes::new())
    }

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(cell_data.pack())
        .cell_dep(sourly_cat_dep)
        .cell_dep(sighash_all_dep)
        .cell_dep(secp_dep)
        .witnesses(witnesses.pack())
        .build();
    let tx = context.complete_tx(tx);
    tx
}

#[test]
fn test_nft_transfer() {
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

    let tx = gen_tx_for_nft_transfer(&mut context, pubkey_hash);

    let tx = sign_tx_keccak256(&mut context, tx, &privkey);
    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}
