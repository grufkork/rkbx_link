use std::process::Command;

use crate::{log::ScopedLogger, y_n};

pub fn rkbx_signing_routine(logger: &ScopedLogger) -> Result<(), ()>{
    match get_entitlements(logger){
        Ok(entitlements) => {
            if entitlements.contains(ENTITLEMENT_GET_TASK_ALLOW){
                logger.info("Rekordbox signed with correct entitlements.");
                Ok(())
            } else {
                logger.warn("Rekordbox is missing the required entitlements.");
                logger.info("Rekordbox must be resigned once in order to read info from it. This requires sudo/admin priviliges, and might show a warning on next launch.");
                if entitlements.is_empty() {
                    logger.warn("Rekordbox appears to be unsigned!");
                    if y_n("Try signing with default entitlements?", false){
                        std::fs::write("entitlements.plist", DEFAULT_ENTITLEMENTS).unwrap();
                    } else {
                        return Err(());
                    }
                } else {
                    if y_n("Add entitlements and resign Rekordbox?", false) {
                        let new_entitlements = add_entitlement(entitlements, ENTITLEMENT_GET_TASK_ALLOW);
                        std::fs::write("entitlements.plist", new_entitlements).unwrap();
                        logger.info("Removing Rekordbox signing");
                    } else {
                        return Err(());
                    }
                }
                let unsign = exec("sudo", &["codesign", "--remove-signature", APPPATH], logger);
                match unsign{
                    Ok(_) => logger.good("Signature removal successful"),
                    Err(e) => {
                        logger.err(&format!("Signature removal failed: {e}"));
                        return Err(());
                    },
                }

                logger.info("Signing Rekordbox with new entitlements...");
                let resign = exec("sudo", &["codesign", "-s", "-", "--deep", "--force", "--entitlements", "entitlements.plist", APPPATH], logger);
                match resign{
                    Ok(res) => {
                        logger.good("Resigning successful");
                        logger.info("Rechecking for entitlements");
                        match get_entitlements(logger) {
                            Ok(entitlements) => {
                                if entitlements.contains(ENTITLEMENT_GET_TASK_ALLOW){
                                    logger.info("Rekordbox signed with correct entitlements.");
                                    logger.warn("Restart Rekordbox if it is running.");
                                    Ok(())
                                }else{
                                    logger.err("Resigning succeeded, but Rekordbox is still missing entitlements!");
                                    Err(())
                                }
                            },
                            Err(e) => {
                                logger.err(&e);
                                Err(())
                            },
                        }
                    },
                    Err(e) => {
                        logger.err(&format!("Resigning failed: {e}"));
                        Err(())
                    },
                }
            }
        },
        Err(e) => {
            logger.err(&e);
            Err(())
        }
    }

}

fn add_entitlement(entitlements: String, new_entitlement: &str) -> String{
	let new = format!("<key>{new_entitlement}</key><true/></dict>");
	entitlements.replace("</dict>", &new)
}

fn get_entitlements(logger: &ScopedLogger) -> Result<String, String> {
	let existing_entitlements = exec("codesign", &["-d", "--entitlements",":-", APPPATH], logger);
    match existing_entitlements{
        Ok(content) => Ok(content.0),
        Err(e) => Err(e),
    }
}

// pub fn check_for_entitlements() -> bool{
//
// 	if existing_entitlements.contains(ENTITLEMENT_GET_TASK_ALLOW) {
// 		println!("get-task-allow already in entitlements");
// 		return true;
// 	}else{
// 		println!("get-task-allow not in entitlements");
// 		return false;
// 	}
// }

const APPPATH: &str = "/Applications/rekordbox 7/rekordbox.app";
const ENTITLEMENT_GET_TASK_ALLOW: &str = "com.apple.security.get-task-allow";

// const ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/><key>com.apple.security.cs.disable-library-validation</key><true/><key>com.apple.security.device.audio-input</key><true/><key>com.apple.security.device.camera</key><true/></dict></plist>"#;

const DEFAULT_ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/><key>com.apple.security.cs.disable-library-validation</key><true/><key>com.apple.security.device.audio-input</key><true/><key>com.apple.security.device.camera</key><true/><key>com.apple.security.get-task-allow</key><true/></dict></plist>"#;

// pub fn resign_rekordbox() -> Result<(), String> {
// 	let res = exec("codesign", &["-d", APPPATH])?;
//
// 	if check_for_entitlements(){
// 		println!("Required entitlements present");
// 	}else{
// 		println!("Missing entitlements");
// 		let mut existing_entitlements = exec("codesign", &["-d", "--entitlements",":-", APPPATH])?.0;
// 		existing_entitlements = add_entitlement(existing_entitlements, ENTITLEMENT_GET_TASK_ALLOW);
// 		println!("Existing: {existing_entitlements}");
//
// 		println!("Denotarising");
// 		
// 		
// 	}
//
// 	
// 	
//
// 	//println!("{res}");
// 	Ok(())
// }



fn exec(cmd: &str, args: &[&str], logger: &ScopedLogger) -> Result<(String, String), String>{
	logger.debug(&format!("Running {cmd} {args:?}"));
	let output = match Command::new(cmd).args(args).output(){
		Ok(res) => res,
		Err(_) => return Err(String::from("AA"))
	};
	
	let stdout = str::from_utf8(&output.stdout).unwrap().to_string();
	let stderr = str::from_utf8(&output.stderr).unwrap().to_string();
    logger.debug(&format!("stdout: {stdout}"));
    logger.debug(&format!("stderr: {stderr}"));

	Ok((stdout, stderr))
}
