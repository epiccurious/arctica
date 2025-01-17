use std::process::Command;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

//import functions from helper
use crate::helper::{
    get_user, get_home, write, generate_keypair, store_string,
};

use crate::bitcoin_wallet::{
    create_wallet, import_descriptor, build_high_descriptor, build_med_descriptor,
	build_low_descriptor,
};

// file paths for this script and create_bootable_usb will need to change for prod
//these paths assume the user is compiling the application with cargo run inside ~/arctica
#[tauri::command]
pub async fn init_iso() -> Result<String, String> {
	println!("Obtaining & creating modified ubuntu iso");
	println!("Removing stale writable");
	//remove writable if exists, developer failsafe
	Command::new("sudo").args(["rm", "-r", "-f", &("/media/".to_string()+&get_user()+"/writable")]).output().unwrap();
	println!("Unmounting stale writable & unbuntu mount if appropriate");
	//remove stale mount points if user has started arctica before
	Command::new("sudo").args(["umount", &("/media/".to_string()+&get_user()+"/Ubuntu 22.04.2 LTS amd64")]).output().unwrap();
	Command::new("sudo").args(["umount", &("/media/".to_string()+&get_user()+"/writable")]).output().unwrap();
	println!("Downloading kvm dependencies");
	//download KVM deps
	Command::new("sudo").args(["apt-get", "-y", "install", "qemu-system-x86", "qemu-kvm", "libvirt-clients", "libvirt-daemon-system", "bridge-utils"]).output().unwrap();
	//obtain mkusb deps, 
	Command::new("sudo").args(["add-apt-repository", "-y", "universe"]).output().unwrap();
	Command::new("sudo").args(["add-apt-repository", "-y", "ppa:mkusb/ppa"]).output().unwrap();
	Command::new("sudo").args(["apt", "-y", "update"]).output().unwrap();
	Command::new("sudo").args(["apt", "install", "-y", "mkusb"]).output().unwrap();
	Command::new("sudo").args(["apt", "install", "-y", "usb-pack-efi"]).output().unwrap();
	//download dependencies required on each Hardware Wallet
	Command::new("sudo").args(["apt", "update"]).output().unwrap();
	Command::new("sudo").args(["apt", "download", "wodim", "genisoimage", "ssss", "qrencode", "libqrencode4", "xclip", "tor"]).output().unwrap();
	//check if ubuntu iso & bitcoin core already exists, and if no, obtain
	//NOTE: this currently checks the arctica repo but this will change once refactor is finished and user can run binary on host machine 
	println!("Obtaining ubuntu iso and bitcoin core if needed");
	let a = std::path::Path::new("./ubuntu-22.04.2-desktop-amd64.iso").exists();
	let b = std::path::Path::new("./bitcoin-25.0-x86_64-linux-gnu.tar.gz").exists();
	if a == false{
		let output = Command::new("wget").args(["-O", "ubuntu-22.04.2-desktop-amd64.iso", "http://releases.ubuntu.com/jammy/ubuntu-22.04.2-desktop-amd64.iso"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in init iso with downloading ubuntu iso = {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
	}
	if b == false{
		let output = Command::new("wget").args(["https://bitcoincore.org/bin/bitcoin-core-25.0/bitcoin-25.0-x86_64-linux-gnu.tar.gz"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in init iso with downloading bitcoin core = {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
	}
	println!("Removing stale persistent ISOs");
	//remove stale persistent isos
	Command::new("sudo").args(["rm", "persistent-ubuntu.iso"]).output().unwrap();
	Command::new("sudo").args(["rm", "persistent-ubuntu1.iso"]).output().unwrap();
	println!("Removing stale pid");
	//remove stale pid file
	Command::new("sudo").args(["rm", "pid.txt"]).output().unwrap();
	println!("Modifying ubuntu iso to have persistence");
	//modify ubuntu iso to have persistence
	let output = Command::new("bash").args([&(get_home()+"/arctica/scripts/sed1.sh")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in running sed1 {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	let exists = Path::new(&(get_home()+"/arctica/persistent-ubuntu1.iso")).exists();
	if !exists {
		return Err(format!("ERROR in running sed1, script completed but did not create iso"));
	}
	//modify ubuntu iso to have a shorter timeout at boot screen
	println!("Modifying ubuntu iso timeout");
	let output = Command::new("bash").args([&(get_home()+"/arctica/scripts/sed2.sh")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in running sed2 {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	let exists = Path::new(&(get_home()+"/arctica/persistent-ubuntu.iso")).exists();
	if !exists {
		return Err(format!("ERROR in running sed2, script completed but did not create iso"));
	}
	println!("Removing stale persistent iso");
	//remove stale persistent iso
	Command::new("sudo").args(["rm", "persistent-ubuntu1.iso"]).output().unwrap();
	println!("fallocate persistent iso");
	//fallocate persistent iso, creates a 7GB image. Image size determines final storage space allocated to writable
	let output = Command::new("fallocate").args(["-l", "7GiB", "persistent-ubuntu.iso"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with fallocate persistent iso = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Booting iso with kvm");
	//boot kvm to establish persistence
	let output = Command::new("kvm").args(["-m", "2048", &(get_home()+"/arctica/persistent-ubuntu.iso"), "-daemonize", "-pidfile", "pid.txt", "-cpu", "host", "-display", "none"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with kvm = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Sleeping for 200 seconds");
	// sleep for 250 seconds
	Command::new("sleep").args(["200"]).output().unwrap();
	println!("Obtaining pid");
	//obtain pid
	let file = "./pid.txt";
	let pid = match fs::read_to_string(file){
		Ok(data) => data.replace("\n", ""),
		Err(err) => return Err(format!("{}", err.to_string()))
	};
	println!("Killing pid");
	//kill pid
	let output = Command::new("kill").args(["-9", &pid]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with killing pid = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Mount persistent iso");
	//mount persistent iso at /media/$USER/
	let output = Command::new("udisksctl").args(["loop-setup", "-f", &(get_home()+"/arctica/persistent-ubuntu.iso")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with mounting persistent iso = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Sleep for 2 seconds");
	// sleep for 2 seconds
	Command::new("sleep").args(["2"]).output().unwrap();
	let a = std::path::Path::new(&("/media/".to_string()+&get_user()+"/writable/")).exists();
	if a == false{
		return Err(format!("ERROR in init iso, problem with initial boot, persistent dir not found at /media/$USER/writable"))
	}
	println!("opening file permissions for persistent dir");
	//open file permissions for persistent directory
	let output = Command::new("sudo").args(["chmod", "777", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with opening file permissions of persistent dir = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Making dependencies directory");
	//make dependencies directory
	Command::new("mkdir").args([&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	println!("Copying dependencies");
	//copying over dependencies genisoimage
	let output = Command::new("cp").args([&(get_home()+"/arctica/genisoimage_9%3a1.1.11-3.2ubuntu1_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying genisoimage = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies ssss
	let output = Command::new("cp").args([&(get_home()+"/arctica/ssss_0.5-5_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying ssss = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies wodim
	let output = Command::new("cp").args([&(get_home()+"/arctica/wodim_9%3a1.1.11-3.2ubuntu1_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying wodim = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies libqrencode4 library
	let output = Command::new("cp").args([&(get_home()+"/arctica/libqrencode4_4.1.1-1_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying qrencode = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies qrencode
	let output = Command::new("cp").args([&(get_home()+"/arctica/qrencode_4.1.1-1_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying qrencode = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies xclip
	let output = Command::new("cp").args([&(get_home()+"/arctica/xclip_0.13-2_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying xclip = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copying over dependencies tor
	let output = Command::new("cp").args([&(get_home()+"/arctica/tor_0.4.6.10-1_amd64.deb"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/dependencies")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying tor = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Copying arctica binary");
	//copy over artica binary and make executable
	let output = Command::new("cp").args([&(get_home()+"/arctica/target/debug/app"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/arctica")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying arctica binary = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Copying arctica icon");
	let output = Command::new("cp").args([&(get_home()+"/arctica/icons/arctica.jpeg"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/arctica.jpeg")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying binary jpeg = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Making arctica a .desktop file");
	let output = Command::new("sudo").args(["cp", &(get_home()+"/arctica/shortcut/Arctica.desktop"), &("/media/".to_string()+&get_user()+"/writable/upper/usr/share/applications/Arctica.desktop")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying arctica.desktop = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//keeping this commented out for dev work due to regular binary swapping
    //make arctica binary autostart after OS boot
	// println!("make arctica autostart at boot");
	// Command::new("mkdir").args([&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/.config/autostart")]).output().unwrap();
	// let output = Command::new("sudo").args(["cp", &(get_home()+"/arctica/shortcut/Arctica.desktop"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/.config/autostart")]).output().unwrap();
	// if !output.status.success() {
	// 	return format!("ERROR in init iso with copying arctica.desktop = {}", std::str::from_utf8(&output.stderr).unwrap());
	// }
	println!("Making arctica binary an executable");
	//make the binary an executable file
	let output = Command::new("sudo").args(["chmod", "+x", &("/media/".to_string()+&get_user()+"/writable/upper/usr/share/applications/Arctica.desktop")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with making binary executable = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Copying scripts library");
	//copy over scripts directory and its contents. 
	let output = Command::new("cp").args(["-r", &(get_home()+"/arctica/scripts"), &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with copying scripts dir = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Extracting bitcoin core");
	//extract bitcoin core
	let output = Command::new("tar").args(["-xzf", &(get_home()+"/arctica/bitcoin-25.0-x86_64-linux-gnu.tar.gz"), "-C", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with extracting bitcoin core = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Creating target device .bitcoin dir");
	//create target device .bitcoin dir
	let output = Command::new("mkdir").args([&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/.bitcoin")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso with making target .bitcoin dir = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	println!("Creating bitcoin.conf on target device");
	//create bitcoin.conf on target device
	let file = File::create(&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/.bitcoin/bitcoin.conf")).unwrap();
	let output = Command::new("echo").args(["-e", "rpcuser=rpcuser\nrpcpassword=477028\nspendzeroconfchange=1"]).stdout(file).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in init iso, with creating bitcoin.conf = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in init_iso"))
}


//initial flash of all 7 Hardware Wallets
//creates a bootable usb stick or SD card that will boot into an ubuntu live system when inserted into a computer
//TODO it would be great if we could find a way to format the usb and remove any partitions before running this
#[tauri::command]
pub async fn create_bootable_usb(number: String, setup: String) -> Result<String, String> {
	//remove any stale config file
	let a = std::path::Path::new(&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")).exists();
	if a == true{
		Command::new("sudo").args(["rm", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")]).output().unwrap();
	}
	//write device type to config, values provided by front end
	let file = File::create(&("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")).unwrap();
	Command::new("echo").args(["type=hardwareWallet\nhwNumber=".to_string()+&number.to_string()+&"\nsetupStep=".to_string()+&setup.to_string()]).stdout(file).output().unwrap();
	println!("creating bootable ubuntu device writing config...HW {} Setupstep {}", number, setup);
	// sleep for 4 seconds
	Command::new("sleep").args(["4"]).output().unwrap();
	//open file permissions for config
	let output = Command::new("sudo").args(["chmod", "777", &("/media/".to_string()+&get_user()+"/writable/upper/home/ubuntu/config.txt")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in creating bootable with opening file permissions = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//burn iso with mkusb
	let mkusb_child = Command::new("printf").args(["%s\n", "n", "y", "g", "y"]).stdout(Stdio::piped()).spawn().unwrap();
	println!("Received stdout, piping to MKUSB");
	let mkusb_child_two = Command::new("mkusb").args([&(get_home()+"/arctica/persistent-ubuntu.iso")]).stdin(Stdio::from(mkusb_child.stdout.unwrap())).stdout(Stdio::piped()).spawn().unwrap();
	println!("MKUSB finished creating output");
	let output = mkusb_child_two.wait_with_output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in creating bootable with mkusb = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in creating bootable device"))
}

#[tauri::command]
//generates a public and private key pair and stores them as a text file
pub async fn generate_store_key_pair(number: String) -> Result<String, String> {
	//number corresponds to currentHW here and is provided by the front end
	let private_key_file = "/mnt/ramdisk/sensitive/private_key".to_string()+&number;
	let public_key_file = "/mnt/ramdisk/sensitive/public_key".to_string()+&number;
	let private_change_key_file = "/mnt/ramdisk/sensitive/private_change_key".to_string()+&number;
	let public_change_key_file = "/mnt/ramdisk/sensitive/public_change_key".to_string()+&number;
    //generate an extended private and public keypair
    let (xpriv, xpub) = match generate_keypair() {
		Ok((xpriv, xpub)) => (xpriv, xpub),
		Err(err) => return Err("ERROR could not generate keypair: ".to_string()+&err.to_string())
	}; 
	//note that change xkeys and standard xkeys are the same but simply given different derviation paths, they are stored seperately for ease of use
	//change keys are assigned /1/* and external keys are assigned /0/*
    //store the xpriv as a file
	match store_string(xpriv.to_string()+"/0/*", &private_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store private key: ".to_string()+&err)
	}
    //store the xpub as a file
	match store_string(xpub.to_string()+"/0/*", &public_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store public key: ".to_string()+&err)
	}
	//store the change_xpriv as a file
	match store_string(xpriv.to_string()+"/1/*", &private_change_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store private change key: ".to_string()+&err)
	}
	//store the change_xpub as a file
	match store_string(xpub.to_string()+"/1/*", &public_change_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store public change key: ".to_string()+&err)
	}
	//make the pubkey dir in the setupCD staging area if it does not already exist
	let a = std::path::Path::new("/mnt/ramdisk/CDROM/pubkeys").exists();
    if a == false{
		let output = Command::new("mkdir").args(["--parents", "/mnt/ramdisk/CDROM/pubkeys"]).output().unwrap();
		if !output.status.success() {
		return Err(format!("ERROR in creating /mnt/ramdisk/CDROM/pubkeys dir {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
	}
	//copy public key to setupCD dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/sensitive/public_key".to_string()+&number), "/mnt/ramdisk/CDROM/pubkeys"]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in generate store key pair with copying pub key= {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//copy public change key to setupCD dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/sensitive/public_change_key".to_string()+&number), "/mnt/ramdisk/CDROM/pubkeys"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying pub change key= {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS generated and stored Private and Public Key Pair"))
}

//this function simulates the creation of a time machine key. Eventually this creation will be performed by the BPS and 
//the pubkeys will be shared with the user instead. 4 Time machine Keys are needed so this function will be run 4 times in total.
//eventually these will need to be turned into descriptors and we will need an encryption scheme for the descriptors/keys that will be held by the BPS so as not to be privacy leaks
//decryption key will be held within encrypted tarball on each Hardware Wallet
#[tauri::command]
pub async fn generate_store_simulated_time_machine_key_pair(number: String) -> Result<String, String> {
	//make the time machine key dir in the setupCD staging area if it does not already exist
	let a = std::path::Path::new("/mnt/ramdisk/CDROM/timemachinekeys").exists();
    if a == false{
		let output = Command::new("mkdir").args(["--parents", "/mnt/ramdisk/CDROM/timemachinekeys"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in creating /mnt/ramdisk/CDROM/timemachinekeys dir {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
	}
	//TODO NOTE THAT THESE KEYS ARE STORED ALL OVER THE PLACE, fine for now but they will need to be properly stored once BPS is integrated
	//number param is provided by the front end
	let private_key_file = "/mnt/ramdisk/CDROM/timemachinekeys/time_machine_private_key".to_string()+&number;
	let public_key_file = "/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_key".to_string()+&number;
	let private_change_key_file = "/mnt/ramdisk/CDROM/timemachinekeys/time_machine_private_change_key".to_string()+&number;
	let public_change_key_file = "/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_change_key".to_string()+&number;
	let (xpriv, xpub) = match generate_keypair() {
		Ok((xpriv, xpub)) => (xpriv, xpub),
		Err(err) => return Err("ERROR could not generate keypair: ".to_string()+&err.to_string())
	};
	//note that change xkeys and standard xkeys are the same but simply given different derviation paths, they are stored seperately for ease of use
	//change keys are assigned /1/* and external keys are assigned /0/*
    //store the xpriv as a file
	match store_string(xpriv.to_string()+"/0/*", &private_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store private key: ".to_string()+&err)
	}
    //store the xpub as a file
	match store_string(xpub.to_string()+"/0/*", &public_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store public key: ".to_string()+&err)
	}
	//store the change_xpriv as a file
	match store_string(xpriv.to_string()+"/1/*", &private_change_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store private change key: ".to_string()+&err)
	}
	//store the change_xpub as a file
	match store_string(xpub.to_string()+"/1/*", &public_change_key_file) {
		Ok(_) => {},
		Err(err) => return Err("ERROR could not store public change key: ".to_string()+&err)
	}
	//copy public key to setupCD dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_key".to_string()+&number), "/mnt/ramdisk/CDROM/pubkeys"]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in generate store key pair with copying pub key to CDROM= {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//copy public change key to setupCD dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_change_key".to_string()+&number), "/mnt/ramdisk/CDROM/pubkeys"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying pub change key to CDROM= {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy public key to sensitive dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_key".to_string()+&number), "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying pub key to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy public change key to sensitive dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_public_change_key".to_string()+&number), "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying pub change key to sensitive= {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy private key to sensitive dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_private_key".to_string()+&number), "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying private key to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy private change key to sensitive dir
	let output = Command::new("cp").args([&("/mnt/ramdisk/CDROM/timemachinekeys/time_machine_private_change_key".to_string()+&number), "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in generate store key pair with copying private change key to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS generated and stored Private and Public Time Machine Key Pair"))
}

//create arctica descriptors
//High Descriptor is the time locked 5 of 11 with decay (4 keys will eventually go to BPS)
//Medium Descriptor is the 2 of 7 with decay
//Low Descriptor is the 1 of 7 and will be used for the tripwire
//acceptable params should be "1", "2", "3", "4", "5", "6", "7"
#[tauri::command]
pub async fn create_descriptor(hwnumber: String) -> Result<String, String> {
   println!("creating descriptors from 7 xpubs & 4 time machine keys");
   //convert all 11 public_keys in the ramdisk to an array vector
   println!("creating key array");
   let mut key_array = Vec::new();
   let mut change_key_array = Vec::new();
   //push the 7 standard public keys into the key_array vector
   println!("pushing 7 standard pubkeys into key array");
   for i in 1..=7{
       let key = match fs::read_to_string(&("/mnt/ramdisk/CDROM/pubkeys/public_key".to_string()+&(i.to_string()))){
        Ok(key)=> key,
        Err(err)=> return Err(format!("{}", err.to_string()))
    };
       key_array.push(key);
       println!("pushed key");
   }
   //push the 4 time machine public keys into the key_array vector, only on HW 1.
	println!("pushing 4 time machine pubkeys into key array");
	for i in 1..=4{
		let key = match fs::read_to_string(&("/mnt/ramdisk/CDROM/pubkeys/time_machine_public_key".to_string()+&(i.to_string()))){
			Ok(key)=> key,
			Err(err)=> return Err(format!("{}", err.to_string()))
		};
		key_array.push(key);
		println!("pushed key");
	}
   println!("printing key array");
   println!("{:?}", key_array);

      //push the 7 public change keys into the change_key_array vector
	  println!("pushing 7 pub change keys into change key array");
	  for i in 1..=7{
		  let key = match fs::read_to_string(&("/mnt/ramdisk/CDROM/pubkeys/public_change_key".to_string()+&(i.to_string()))){
		   Ok(key)=> key,
		   Err(err)=> return Err(format!("{}", err.to_string()))
	   };
		  change_key_array.push(key);
		  println!("pushed key");
	  }
	  //push the 4 time machine public keys into the key_array vector, only on HW 1.
	   println!("pushing 4 time machine pub change keys into change key array");
	   for i in 1..=4{
		   let key = match fs::read_to_string(&("/mnt/ramdisk/CDROM/pubkeys/time_machine_public_change_key".to_string()+&(i.to_string()))){
			   Ok(key)=> key,
			   Err(err)=> return Err(format!("{}", err.to_string()))
		   };
		   change_key_array.push(key);
		   println!("pushed key");
	   }
	println!("printing change key array");
   	println!("{:?}", change_key_array);

   //create the descriptors directory inside of ramdisk
   println!("Making descriptors dir");
   Command::new("mkdir").args(["/mnt/ramdisk/sensitive/descriptors"]).output().unwrap();

   //build the delayed wallet descriptor
   println!("building high descriptor");
   let high_descriptor = match build_high_descriptor(&key_array, &hwnumber, false) {
	Ok(desc) => desc,
	Err(err) => return Err("ERROR could not build High Descriptor ".to_string()+&err)
   };
   //verify that the descriptor did not fail
   if high_descriptor.contains("No such file or directory") {
		return Err("ERROR could not build High Descriptor".to_string())
   }
   //store the delayed wallet descriptor in the sensitive dir
   let high_file_dest = &("/mnt/ramdisk/sensitive/descriptors/delayed_descriptor".to_string()+&hwnumber.to_string()).to_string();
   println!("storing high descriptor");
   match store_string(high_descriptor.to_string(), high_file_dest) {
       Ok(_) => {},
       Err(err) => return Err("ERROR could not store High Descriptor: ".to_string()+&err)
   };
   //build delayed wallet change descriptor
   println!("building high change descriptor");
   let high_change_descriptor = match build_high_descriptor(&change_key_array, &hwnumber, true) {
	Ok(desc) => desc,
	Err(err) => return Err("ERROR could not build High Change Descriptor ".to_string()+&err)
   };
   let high_change_file_dest = &("/mnt/ramdisk/sensitive/descriptors/delayed_change_descriptor".to_string()+&hwnumber.to_string()).to_string();
   //store the delayed wallet change descriptor in the sensitive dir
   println!("storing high change descriptor");
   match store_string(high_change_descriptor.to_string(), high_change_file_dest) {
       Ok(_) => {},
       Err(err) => return Err("ERROR could not store High Change Descriptor: ".to_string()+&err)
   };
   //create the delayed wallet
   println!("creating delayed wallet");
   match create_wallet("delayed".to_string(), &hwnumber){
	Ok(_) => {},
	Err(err) => return Err("ERROR could not create Delayed Wallet: ".to_string()+&err)
   };
   //import the delayed wallet descriptor
   println!("importing delayed descriptor");
   match import_descriptor("delayed".to_string(), &hwnumber, false){
	Ok(_) => {},
	Err(err) => return Err("ERROR could not import Delayed Descriptor: ".to_string()+&err)
   };
	//import delayed change descriptor
	println!("importing delayed change descriptor");
	match import_descriptor("delayed".to_string(), &hwnumber, true){
	Ok(_) => {},
	Err(err) => return Err("ERROR could not import Delayed change Descriptor: ".to_string()+&err)
	};

   //build the immediate wallet descriptor
   println!("building med descriptor");
   let med_descriptor = match build_med_descriptor(&key_array, &hwnumber, false) {	
	Ok(desc) => desc,
	Err(err) => return Err("ERROR could not build Immediate Descriptor ".to_string()+&err)
   };
   	//verify that the descriptor did not fail
	if med_descriptor.contains("No such file or directory") {
		return Err("ERROR could not build Med Descriptor".to_string())
	}
   //store the immediate wallet descriptor in the sensitive dir
   let med_file_dest = &("/mnt/ramdisk/sensitive/descriptors/immediate_descriptor".to_string()+&hwnumber.to_string()).to_string();
   println!("storing med descriptor");
   match store_string(med_descriptor.to_string(), med_file_dest) {
       Ok(_) => {},
       Err(err) => return Err("ERROR could not store Immediate Descriptor: ".to_string()+&err)
   };
   //build the immediate change descriptor
   println!("building med change descriptor");
   let med_change_descriptor = match build_med_descriptor(&change_key_array, &hwnumber, true) {
	Ok(desc) => desc,
	Err(err) => return Err("ERROR could not build Immediate Change Descriptor ".to_string()+&err)
   };
   let med_change_file_dest = &("/mnt/ramdisk/sensitive/descriptors/immediate_change_descriptor".to_string()+&hwnumber.to_string()).to_string();
   //store the immediate change descriptor
   println!("storing med change descriptor");
   match store_string(med_change_descriptor.to_string(), med_change_file_dest) {
       Ok(_) => {},
       Err(err) => return Err("ERROR could not store Immediate Change Descriptor: ".to_string()+&err)
   };
   //create the immediate wallet
   println!("creating immediate wallet");
   match create_wallet("immediate".to_string(), &hwnumber){
	Ok(_) => {},
	Err(err) => return Err("ERROR could not create Immediate Wallet: ".to_string()+&err)
   };
   //import the immediate wallet descriptor
   println!("importing immediate descriptor");
   match import_descriptor("immediate".to_string(), &hwnumber, false){
	Ok(_) => {},
	Err(err) => return Err(format!("ERROR could not import Immediate Descriptor: {}", err))
   };
	//import immediate change descriptor
	println!("importing immediate change descriptor");
	match import_descriptor("immediate".to_string(), &hwnumber, true){
	Ok(_) => {},
	Err(err) => return Err("ERROR could not import Immediate change Descriptor: ".to_string()+&err)
	};

//    //build the low security descriptor
//    println!("building low descriptor");
//    let low_descriptor = match build_low_descriptor(&key_array, &hwnumber, false) {
// 	Ok(desc) => desc,
// 	Err(err) => return Err("ERROR could not build Low Descriptor ".to_string()+&err)
//    };
//    let low_file_dest = &("/mnt/ramdisk/sensitive/descriptors/low_descriptor".to_string()+&hwnumber.to_string()).to_string();
//    //store the low security descriptor in the sensitive dir
//    println!("storing low descriptor");
//    match store_string(low_descriptor.to_string(), low_file_dest) {
//        Ok(_) => {},
//        Err(err) => return Err("ERROR could not store Low Descriptor: ".to_string()+&err)
//    };

//    //build the low change descriptor
//    println!("building low change descriptor");
//    let low_change_descriptor = match build_low_descriptor(&change_key_array, &hwnumber, true) {
// 	Ok(desc) => desc,
// 	Err(err) => return Err("ERROR could not build Low Change Descriptor ".to_string()+&err)
//    };
//    let low_change_file_dest = &("/mnt/ramdisk/sensitive/descriptors/low_change_descriptor".to_string()+&hwnumber.to_string()).to_string();
//    //TODO store the low change descriptor
//    println!("storing low change descriptor");
//    match store_string(low_change_descriptor.to_string(), low_change_file_dest) {
//        Ok(_) => {},
//        Err(err) => return Err("ERROR could not store Low Change Descriptor: ".to_string()+&err)
//    };
//    //creating low wallet
//    println!("creating low wallet");
//    match create_wallet("low".to_string(), &hwnumber){
// 	Ok(_) => {},
// 	Err(err) => return Err("ERROR could not create Low Wallet: ".to_string()+&err)
//    };
//    //importing low descriptor
//    println!("importing low descriptor");
//    match import_descriptor("low".to_string(), &hwnumber, false){
// 	Ok(_) => {},
// 	Err(err) => return Err("ERROR could not import Low Descriptor: ".to_string()+&err)
//    };
//    //import low change descriptor
//    println!("importing low change descriptor");
//    match import_descriptor("low".to_string(), &hwnumber, true){
// 	Ok(_) => {},
// 	Err(err) => return Err("ERROR could not import Low change Descriptor: ".to_string()+&err)
//    };

   println!("Success");
   Ok(format!("SUCCESS in creating descriptors"))
}

//function that creates the setupCD used to pass state between Hardware Wallets
#[tauri::command]
pub async fn create_setup_cd() -> Result<String, String> {
	//query /dev/sr?
	//TODO This check is broken and currently returns an error everytime even when /dev/sr0 is present. Not sure why.
	// let query = Command::new("ls").arg("/dev/sr?").output().unwrap();
	// let query_str = std::str::from_utf8(&query.stderr).unwrap();
	// if query_str.contains("No such file or directory"){
	// 	return Err(format!("ERROR No CD found in create_setup_cd"));
	// }
	println!("creating setup CD");
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
	//install HW dependencies for genisoimage
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/genisoimage_9%3a1.1.11-3.2ubuntu1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing genisoimage for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for ssss
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/ssss_0.5-5_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing ssss for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for wodim
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/wodim_9%3a1.1.11-3.2ubuntu1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing wodim for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install library for qrencode
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/libqrencode4_4.1.1-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing qrencode for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for qrencode
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/qrencode_4.1.1-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing qrencode for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for tor
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/tor_0.4.6.10-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing qrencode for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//create setupCD config
	let file = File::create("/mnt/ramdisk/CDROM/config.txt").unwrap();
	Command::new("echo").args(["type=setupcd" ]).stdout(file).output().unwrap();
	//create masterkey and derive shards
	let output = Command::new("bash").args([&(get_home()+"/scripts/create-setup-cd.sh")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in running create-setup-cd.sh {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//TODO: EVENTUALLY THE APPROPRIATE SHARDS NEED TO GO TO THE BPS HERE

	//copy first 2 shards to HW 1
	let output = Command::new("sudo").args(["cp", "/mnt/ramdisk/shards/shard1.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in copying shard1.txt in create setup CD = {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	let output = Command::new("sudo").args(["cp", "/mnt/ramdisk/shards/shard11.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in copying shard11.txt in create setup CD = {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//remove stale shard file
	let output = Command::new("sudo").args(["rm", "/mnt/ramdisk/shards_untrimmed.txt"]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in removing deprecated shards_untrimmed in create setup cd = {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//stage setup CD dir with shards for distribution
	let output = Command::new("sudo").args(["cp", "-R", "/mnt/ramdisk/shards", "/mnt/ramdisk/CDROM"]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in copying shards to CDROM dir in create setup cd = {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//create the decay directory
	Command::new("mkdir").args(["/mnt/ramdisk/CDROM/decay"]).output().unwrap();
	//create start time file
	let start_time = Command::new("date").args(["+%s"]).output().unwrap();
	let start_time_output = std::str::from_utf8(&start_time.stdout).unwrap();
	let start_time_int = &start_time_output.trim().parse().unwrap();
	//these are the decay times as advertised in documentation
	// delayed_decay1
	let four_years: i32 = start_time_int + 126144000; //start_time + 4 years in seconds
	//delayed_decay2
	let four_years_two_months: i32 = start_time_int + 126144000 + 5184000; //start_time + 4 years in seconds + 2 months in seconds
	//delayed_decay3
	let four_years_four_months: i32 = start_time_int + 126144000 + 10368000; //start_time + 4 years in seconds + 4 months in seconds
	//delayed_decay4
	let four_years_six_months: i32 = start_time_int + 126144000 + 15552000; //start_time + 4 years in seconds + 6 months in seconds
	//delayed_decay5 == immediate_decay
	let four_years_eight_months: i32 = start_time_int + 126144000 + 20736000; //start_time + 4 years in seconds + 8 months in seconds

	//test times, consider adding a prompt at initial setup or a debug console button for changing this set of vars on the fly during initial setup
	// //delayed_decay1
	// let four_years: i32 = start_time_int + 172800; //start_time + 2 days in seconds
	// //delayed_decay2
	// let four_years_two_months: i32 = start_time_int + 172800 + 86400; //start_time + 2 days in seconds + 1 day in seconds
	// //delayed_decay3
	// let four_years_four_months: i32 = start_time_int + 172800 + 172800; //start_time + 2 days in seconds + 2 days in seconds
	// //delayed_decay4
	// let four_years_six_months: i32 = start_time_int + 172800 + 259200; //start_time + 2 days in seconds + 3 days in seconds
	// //delayed_decay5 == immediate_decay
	// let four_years_eight_months: i32 = start_time_int + 172800 + 345600; //start_time + 2 days in seconds + 4 days in seconds

	//store start_time unix timestamp in the decay dir
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/start_time") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create start time file")),
	};
	file_ref.write_all(&start_time_output.to_string().as_bytes()).expect("could not write start_time to file");
	//store delayed_decay1
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/delayed_decay1") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create delayed_decay1 file")),
	};
	file_ref.write_all(&four_years.to_string().as_bytes()).expect("could not write delayed_decay1 to file");
	//store delayed_decay2
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/delayed_decay2") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create delayed_decay2 file")),
	};
	file_ref.write_all(&four_years_two_months.to_string().as_bytes()).expect("could not write delayed_decay2 to file");
	//store delayed_decay3
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/delayed_decay3") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create delayed_decay3 file")),
	};
	file_ref.write_all(&four_years_four_months.to_string().as_bytes()).expect("could not write delayed_decay3 to file");
	//store delayed_decay4
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/delayed_decay4") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create delayed_decay4 file")),
	};
	file_ref.write_all(&four_years_six_months.to_string().as_bytes()).expect("could not write delayed_decay4 to file");
	//store delayed_decay5
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/delayed_decay5") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create delayed_decay5 file")),
	};
	file_ref.write_all(&four_years_eight_months.to_string().as_bytes()).expect("could not write delayed_decay5 to file");
	//store immediate_decay/delayed_decay6 unix timestamp in the decay dir
	let mut file_ref = match std::fs::File::create("/mnt/ramdisk/CDROM/decay/immediate_decay") {
		Ok(file) => file,
		Err(_) => return Err(format!("Could not create immediate_decay file")),
	};
	file_ref.write_all(&four_years_eight_months.to_string().as_bytes()).expect("could not write immediate_decay to file");
	//copy decay dir to sensitive
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
    	return Err(format!("ERROR in copying decay dir from CDROM dir to sensitive dir= {}", std::str::from_utf8(&output.stderr).unwrap()));
    }
	//create iso from setupCD dir
	let output = Command::new("genisoimage").args(["-r", "-J", "-o", "/mnt/ramdisk/setupCD.iso", "/mnt/ramdisk/CDROM"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR refreshing setupCD with genisoimage = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	Command::new("sudo").args(["wodim", "-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();

	//burn setupCD iso to the setupCD
	let output = Command::new("sudo").args(["wodim", "dev=/dev/sr0", "-v", "-data", "/mnt/ramdisk/setupCD.iso"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in refreshing setupCD with burning iso = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//eject the disc
	let output = Command::new("sudo").args(["eject", "/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in Creating Setup CD"))
}

#[tauri::command]
//install dependencies manually from files on each of the offline Hardware Wallets (2-7)
pub async fn install_hw_deps() -> Result<String, String> {
	println!("installing deps required by Hardware Wallet");
	//these are required on all 7 Hardware Wallets
	//install HW dependencies for genisoimage
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/genisoimage_9%3a1.1.11-3.2ubuntu1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing genisoimage {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for ssss
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/ssss_0.5-5_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing ssss {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for wodim
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/wodim_9%3a1.1.11-3.2ubuntu1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing wodim {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install library for qrencode
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/libqrencode4_4.1.1-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing qrencode for create_setup_cd {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for qrencode
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/qrencode_4.1.1-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing qrencode {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for xclip
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/xclip_0.13-2_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing xclip {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	//install HW dependencies for Tor
	let output = Command::new("sudo").args(["apt", "install", &(get_home()+"/dependencies/tor_0.4.6.10-1_amd64.deb")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in installing tor {}", std::str::from_utf8(&output.stderr).unwrap()));
	} 
	Ok(format!("SUCCESS in installing HW dependencies"))
}

//The following "distribute_shards" fuctions are for distributing encryption key shards to each HW 2-7
#[tauri::command]
pub async fn distribute_shards_hw2() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard2.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 2 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
    //copy the shards to the target destination
	//this is a copy of a BPS shard
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard10.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 2 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 2"))
}

#[tauri::command]
pub async fn distribute_shards_hw3() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard3.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 3 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
    //copy the shards to the target destination
	//this is a copy of a BPS shard 
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard9.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 3 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 3"))
}

#[tauri::command]
pub async fn distribute_shards_hw4() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard4.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 4 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
    //copy the shards to the target destination 
	//this is a copy of a BPS shard
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard8.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 4 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 4"))
}

#[tauri::command]
pub async fn distribute_shards_hw5() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard5.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 5 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 5"))
}

#[tauri::command]
pub async fn distribute_shards_hw6() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard6.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 6 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 6"))
}

#[tauri::command]
pub async fn distribute_shards_hw7() -> Result<String, String> {
	//create local shards dir
	Command::new("mkdir").args([&(get_home()+"/shards")]).output().unwrap();
    //copy the shards to the target destination
	let output = Command::new("cp").args(["/mnt/ramdisk/CDROM/shards/shard7.txt", &(get_home()+"/shards")]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in distributing shards to HW 7 = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	//copy the time_decay directory
	let output = Command::new("cp").args(["-r", "/mnt/ramdisk/CDROM/decay", "/mnt/ramdisk/sensitive"]).output().unwrap();
	if !output.status.success() {
		return Err(format!("ERROR in copying decay dir to sensitive = {}", std::str::from_utf8(&output.stderr).unwrap()));
	}
	Ok(format!("SUCCESS in distributing shards to HW 7"))
}

//Create a backup directory of the currently inserted Hardware Wallet
#[tauri::command]
pub async fn create_backup(number: String) -> Result<String, String> {
	println!("creating backup directory of the current HW");
		//make backup dir for iso
		Command::new("mkdir").args(["/mnt/ramdisk/backup"]).output().unwrap();
		//Copy shards to backup
		let output = Command::new("cp").args(["-r", &(get_home()+"/shards"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in creating backup with copying shards = {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
		//Copy sensitive dir
		let output = Command::new("cp").args([&(get_home()+"/encrypted.gpg"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in creating backup with copying sensitive dir= {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
		//copy config
		let output = Command::new("cp").args([&(get_home()+"/config.txt"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in creating backup with copying config.txt= {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
		//create .iso from backup dir
		let output = Command::new("genisoimage").args(["-r", "-J", "-o", &("/mnt/ramdisk/backup".to_string()+&number+".iso"), "/mnt/ramdisk/backup"]).output().unwrap();
		if !output.status.success() {
			return Err(format!("ERROR in creating backup with creating iso= {}", std::str::from_utf8(&output.stderr).unwrap()));
		}
	
		Ok(format!("SUCCESS in creating backup of current HW"))
}

//make the existing backup directory into an iso and burn to the currently inserted CD/DVD/M-DISC
//TODO this function does not currently trigger error handlers (so users can skip back up CDs and DVDs if desired for testing)
#[tauri::command]
pub async fn make_backup(number: String) -> String {
	println!("making backup iso of the current HW and burning to CD");
	// sleep for 4 seconds
	Command::new("sleep").args(["4"]).output().unwrap();
	//wipe the CD
	Command::new("sudo").args(["umount", "/dev/sr0"]).output().unwrap();
	//we don't mind if this fails, CD-Rs will fail this script always
	Command::new("sudo").args(["wodim", "-v", "dev=/dev/sr0", "blank=fast"]).output().unwrap();

	//burn setupCD iso to the backup CD
	let output = Command::new("sudo").args(["wodim", "dev=/dev/sr0", "-v", "-data", &("/mnt/ramdisk/backup".to_string()+&number+".iso")]).output().unwrap();
	if !output.status.success() {
		return format!("ERROR in making backup with burning iso to CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}
	//eject the disc
	let output = Command::new("sudo").args(["eject", "/dev/sr0"]).output().unwrap();
	if !output.status.success() {
		return format!("ERROR in refreshing setupCD with ejecting CD = {}", std::str::from_utf8(&output.stderr).unwrap());
	}

	format!("SUCCESS in making backup of current HW")
}