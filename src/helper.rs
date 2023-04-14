use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth, Client, Error, RawTx};
use bitcoincore_rpc::bitcoincore_rpc_json::{AddressType, ImportDescriptors, Timestamp};
use bitcoincore_rpc::bitcoincore_rpc_json::{GetRawTransactionResult, WalletProcessPsbtResult, CreateRawTransactionInput, ListTransactionResult, Bip125Replaceable, GetTransactionResultDetailCategory, WalletCreateFundedPsbtOptions, WalletCreateFundedPsbtResult, FinalizePsbtResult};
use bitcoin;
use bitcoin::locktime::Time;
use bitcoin::Address;
use bitcoin::consensus::serialize;
use bitcoin::consensus::deserialize;
use bitcoin::psbt::PartiallySignedTransaction;
use bitcoin::util::bip32::ExtendedPubKey;
use bitcoin::util::bip32::ExtendedPrivKey;
use bitcoin::util::amount::SignedAmount;
use bitcoin::Amount;
use bitcoin::Txid;
use bitcoin::Transaction;
use bitcoin::psbt::Psbt;
use std::sync::{Arc, Mutex};
use std::ops::Deref;
use std::process::Command;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;
use std::collections::BTreeMap;
use home::home_dir;
use secp256k1::{rand, Secp256k1, SecretKey};
use secp256k1::rand::Rng;
use tauri::State;
use std::{thread, time::Duration};
use std::path::Path;
use std::process::Stdio;
use std::io::BufReader;
use std::any::type_name;
use std::num::ParseIntError;
use hex;
use serde_json::{json, to_string, Value};
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::mem;
use base64::decode;

//helper function
//get the current user
pub fn get_user() -> String {
	home_dir().unwrap().to_str().unwrap().to_string().split("/").collect::<Vec<&str>>()[2].to_string()
}

//helper function
//only useful when running the application in a dev envrionment
//prints & error messages must be passed to the front end in a promise when running from a precompiled binary
pub fn print_rust(data: &str) -> String {
	println!("input = {}", data);
	format!("completed with no problems")
}

//helper function
//determine the data type of the provided variable
pub fn type_of<T>(_: &T) -> &'static str{
	type_name::<T>()
}


//helper function
//get the current $HOME path
pub fn get_home() -> String {
	home_dir().unwrap().to_str().unwrap().to_string()
}

//helper function
//check for the presence of an internal storage uuid and if one is mounted, return it
pub fn get_uuid() -> String {
	//Obtain the internal storage device UUID if mounted
	let devices = Command::new(&("ls")).args([&("/media/".to_string()+&get_user())]).output().unwrap();
	if !devices.status.success() {
	return format!("ERROR in parsing /media/user");
	} 
	//convert the list of devices above into a vector of results
	let devices_output = std::str::from_utf8(&devices.stdout).unwrap();
	let split = devices_output.split('\n');
	let devices_vec: Vec<_> = split.collect();
	//loop through the vector and check the character count of each entry to obtain the uuid which is 36 characters
	let mut uuid = "none";
	for i in devices_vec{
		if i.chars().count() == 36{
			uuid = i.trim();
		} 
	}
	//if a valid uuid is not found, this function returns the string: "none"
	format!("{}", uuid)
}

//helper function
//check if target path is empty
pub fn is_dir_empty(path: &str) -> bool {
	if let Ok(mut entries) = fs::read_dir(path){
		return entries.next().is_none();
	}
	false
}

//helper function
//used to store keypairs & descriptors as a file
pub fn store_string(string: String, file_name: &String) -> Result<String, String> {
	let mut fileRef = match std::fs::File::create(file_name) {
		Ok(file) => file,
		Err(err) => return Err(err.to_string()),
	};
	fileRef.write_all(&string.as_bytes());
	Ok(format!("SUCCESS stored with no problems"))
}

//helper function
//used to store the generated PSBT as a file
pub fn store_psbt(psbt: &WalletProcessPsbtResult, file_name: String) -> Result<String, String> {
    let mut fileRef = match std::fs::File::create(file_name) {
        Ok(file) => file,
        Err(err) => return Err(err.to_string()),
    };
    let psbt_json = to_string(&psbt).unwrap();
    fileRef.write_all(&psbt_json.to_string().as_bytes());
    Ok(format!("SUCCESS stored with no problems"))
 }

//helper function
//copy any shards potentially on the recovery CD to ramdisk
pub fn copy_shards_to_ramdisk() {
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard1.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard2.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard3.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard4.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard5.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard6.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard7.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard8.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard9.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard10.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
	Command::new("cp").args([&("/media/".to_string()+&get_user()+"/CDROM/shards/shard11.txt"), "/mnt/ramdisk/shards"]).output().unwrap();
}

//helper function
//update the config.txt with the provided params
pub fn write(name: String, value:String) {
	let mut config_file = home_dir().expect("could not get home directory");
    config_file.push("config.txt");
    let mut written = false;
    let mut newfile = String::new();

    let contents = match fs::read_to_string(&config_file) {
        Ok(ct) => ct,
        Err(_) => {
            "".to_string()       
        }
    };

    for line in contents.split("\n") {
        let parts: Vec<&str> = line.split("=").collect();
        if parts.len() == 2 {
           let (n,v) = (parts[0],parts[1]); 
           newfile += n;
           newfile += "=";
           if n == name {
            newfile += &value;
            written = true;
           } else {
            newfile += v;
           }
           newfile += "\n";
        }
    }

    if !written {
        newfile += &name;
        newfile += "=";
        newfile += &value;
    }

    let mut file = File::create(&config_file).expect("Could not Open file");
    file.write_all(newfile.as_bytes()).expect("Could not rewrite file");
}


//helper function
//used to check the mountpoint of /media/$USER/CDROM
pub fn check_cd_mount() -> std::string::String {
	let mut mounted = "false";
	let output = Command::new("df").args(["-h", &("/media/".to_string()+&get_user()+"/CDROM")]).output().unwrap();
	if !output.status.success() {
		let er = "error";
		return format!("{}", er)
	}
		
	let df_output = std::str::from_utf8(&output.stdout).unwrap();
	//use a closure to split the output of df -h /media/$USER/CDROM by whitespace and \n
	let split = df_output.split(|c| c == ' ' || c == '\n');
	let output_vec: Vec<_> = split.collect();
	//loop through the vector
	for i in output_vec{
		println!("new line:");
		println!("{}", i);
		//if any of the lines contain /dev/sr0 we know that /media/$USER/CDROM is mounted correctly
		if i == "/dev/sr0"{
			mounted = "true";
			return format!("success")
		}
	}
	if mounted == "false"{
		//check if filepath exists
		let b = std::path::Path::new(&("/media/".to_string()+&get_user()+"/CDROM")).exists();
		//if CD mount path does not exist...create it and mount the CD
		if b == false{
			let output = Command::new("sudo").args(["mkdir", &("/media/".to_string()+&get_user()+"/CDROM")]).output().unwrap();
				if !output.status.success() {
					return format!("error");
				}
			let output = Command::new("sudo").args(["mount", "/dev/sr0", &("/media/".to_string()+&get_user()+"/CDROM")]).output().unwrap();
			if !output.status.success() {
				return format!("error");
			}
		//if CD mount path already exists...mount the CD
		} else {
			let output = Command::new("sudo").args(["mount", "/dev/sr0", &("/media/".to_string()+&get_user()+"/CDROM")]).output().unwrap();
				if !output.status.success() {
					return format!("error");
				}
		}
	}
	format!("success")
}

//helper function
//used to generate an extended public and private keypair
pub fn generate_keypair() -> Result<(String, String), bitcoin::Error> {
	let secp = Secp256k1::new();
    let seed = SecretKey::new(&mut rand::thread_rng()).secret_bytes();
    let xpriv = bitcoin::util::bip32::ExtendedPrivKey::new_master(bitcoin::Network::Bitcoin, &seed).unwrap();
	let xpub = bitcoin::util::bip32::ExtendedPubKey::from_priv(&secp, &xpriv);
	Ok((bitcoin::util::base58::check_encode_slice(&xpriv.encode()), bitcoin::util::base58::check_encode_slice(&xpub.encode())))
}
