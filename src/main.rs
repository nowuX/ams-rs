use dialoguer::Input;
use log::{debug, error, info, warn};
use owo_colors::OwoColorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    env::{consts, current_dir, set_current_dir},
    fs,
    io::{self, BufRead, Write},
    path, process,
};

fn subprocess_logger(prog: &str, args: Vec<&str>) -> Result<process::Output, io::Error> {
    let process = process::Command::new(prog).args(args).output();

    fn print_output(output: &Vec<u8>) {
        String::from_utf8(output.to_vec())
            .unwrap()
            .lines()
            .for_each(|i| {
                debug!("{}", i);
            })
    }
    let out = process?;
    print_output(&out.stdout);
    print_output(&out.stderr);
    Ok(out)
}

fn i(msg: &str) -> String {
    format!(" {} {}", "INPUT".blue(), msg)
}

fn subprocess(prog: &str, args: Vec<&str>) -> Result<process::Output, io::Error> {
    let process = process::Command::new(prog).args(args).output();
    let out = process?;
    Ok(out)
}

/// Function to validate each script requirement
fn check_environment() -> String {
    debug!("Check environment...");
    let py_cmd = match consts::OS {
        "windows" => "py",
        "linux" => "python3",
        _ => {
            error!("OS {} is currently not supported", consts::OS);
            process::exit(1)
        }
    };

    if subprocess_logger("java", vec!["-version"]).is_err() {
        warn!("Java is needed");
        error!("System can't find java");
        process::exit(1);
    }

    let pip_packages = subprocess(py_cmd, vec!["-m", "pip", "list", "--format", "freeze"]).unwrap();
    let pip_list = String::from_utf8(pip_packages.stdout).unwrap();
    let pip_list = pip_list
        .lines()
        .map(|x| x.split("==").next().unwrap())
        .collect::<Vec<&str>>();
    match pip_list.iter().find(|x| **x == "mcdreforged") {
        Some(_) => {}
        None => {
            warn!("MCDReforged package not detected");
            info!("Installing MCDReforged...");
            subprocess_logger(py_cmd, vec!["-m", "pip", "install", "mcdreforged"]).unwrap();
        }
    }
    return py_cmd.to_owned();
}

/// Create a folder for server install
fn mk_folder() {
    let re = Regex::new(r"\W").unwrap();

    let input: String = Input::new()
        .with_prompt(i("Enter the server folder name"))
        .default("minecraft_server".into())
        .interact_text()
        .unwrap();
    let mut folder: String = String::from(re.replace(&input.trim().replace(" ", "_"), ""));

    if folder.is_empty() {
        folder = String::from("minecraft_server");
    }
    if path::Path::new(&folder).exists() {
        warn!("Folder already exists");
        process::exit(0);
    }
    match fs::create_dir(path::Path::new(&folder)) {
        Ok(_) => {
            let mut path = current_dir().unwrap();
            path.push(folder);
            info!("Making directory: {}", path.display());
            set_current_dir(&path).unwrap();
        }
        Err(e) => {
            error!("Something failed while the folder was being create: {}", e);
            process::exit(1);
        }
    };
}

/// Function to choose the server loader
fn server_mod_loader() -> i8 {
    info!("Which looder do you want to use?");
    let nose = [(1, "Vanilla"), (2, "Fabric")];
    for (k, v) in nose {
        info!("{} | {}", k, v);
    }
    loop {
        let input: String = Input::new()
            .with_prompt(i("Select a option"))
            .interact_text()
            .unwrap();
        return match input.to_lowercase().as_str() {
            "1" | "vanilla" => 1,
            "2" | "fabric" => 2,
            _ => continue,
        };
    }
}

///Make a simple yes or no question
fn simple_yes_no(question: &str, default_yes: bool) -> bool {
    loop {
        let choices = match default_yes {
            true => "[Y/n]",
            false => "[y/N]",
        };
        let input: String = Input::new()
            .with_prompt(i(&format!("{} {}", question, choices)))
            .allow_empty(true)
            .interact_text()
            .unwrap();
        return match input.to_lowercase().trim() {
            "" => default_yes,
            "yes" | "y" => true,
            "no" | "n" => false,
            &_ => panic!(),
        };
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Latest {
    release: String,
    snapshot: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<Version>,
}

/// Get last Minecraft Server Release
fn get_last_release() -> String {
    reqwest::blocking::get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
        .unwrap()
        .json::<VersionManifest>()
        .unwrap()
        .latest
        .release
}

#[derive(Debug, Deserialize)]
struct DownloadsMapping {
    url: String,
}

#[derive(Debug, Deserialize)]
struct Downloads {
    server: DownloadsMapping,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    downloads: Downloads,
}

/// Function to install the vanilla loader
fn vanilla_loader() -> [String; 2] {
    debug!("Vanilla loader setup");
    loop {
        let input: String = Input::new()
            .with_prompt(i("Which minecraft version do you want to use? [latest]"))
            .allow_empty(true)
            .interact_text()
            .unwrap();
        let minecraft = match input.trim().is_empty() {
            true => get_last_release(),
            false => input.trim().to_owned(),
        };
        let tmp: Vec<i8> = minecraft
            .split(".")
            .map(|x| x.parse::<i8>().unwrap())
            .collect();
        let major = tmp[1];
        let minor = match tmp.len() == 3 {
            true => tmp[2],
            false => 0,
        };
        if major < 2 || (major == 2 && minor < 5) {
            error!("This version is currently unsupported by the script");
            process::exit(1);
        }

        let re = Regex::new(r"[\d.]").unwrap();
        if !re.is_match(&minecraft) {
            warn!("Version provided contain invalid characters");
            continue;
        }

        info!("Version selected: {}", &minecraft);
        info!("Dowloading vanilla loader...");
        let versions_json = reqwest::blocking::get(
            "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
        )
        .unwrap()
        .json::<VersionManifest>()
        .unwrap()
        .versions;

        for v in versions_json {
            if v.id != minecraft {
                continue;
            }
            let url = v.url;
            debug!("url={}", url);
            let server_url = reqwest::blocking::get(&url)
                .unwrap()
                .json::<VersionData>()
                .unwrap()
                .downloads
                .server
                .url;
            let server_file = server_url.split("/").collect::<Vec<&str>>()[6];
            let response = reqwest::blocking::get(&server_url)
                .unwrap()
                .bytes()
                .unwrap();
            let mut file = fs::File::create(server_file).unwrap();
            let mut content = io::Cursor::new(response);
            io::copy(&mut content, &mut file).unwrap();
            info!("Vanilla server installation complete");
            return [server_file.replace(".jar", ""), minecraft];
        }
        warn!("Version not found");
    }
}

/// Function to install the Fabric Loader
fn fabric_loader() -> [String; 2] {
    debug!("Fabric Loader setup");
    const FABRIC_URL: &str = "https://maven.fabricmc.net/net/fabricmc/fabric-installer/0.11.0/fabric-installer-0.11.0.jar";

    let installer = FABRIC_URL.split("/").collect::<Vec<&str>>()[7];
    info!("Downloading fabric loader...");
    let response = reqwest::blocking::get(FABRIC_URL).unwrap().bytes().unwrap();
    let mut file = fs::File::create(installer).unwrap();
    let mut content = io::Cursor::new(response);
    io::copy(&mut content, &mut file).unwrap();

    #[allow(unused_assignments)]
    let mut minecraft = String::new();
    loop {
        let input: String = Input::new()
            .with_prompt(i("Which version of Minecraft do you want to use? [latest]"))
            .allow_empty(true)
            .interact_text()
            .unwrap();
        minecraft = input.trim().to_owned();
        let re = Regex::new(r"[^\d.]").unwrap();
        if !minecraft.is_empty() && re.is_match(&minecraft) {
            warn!("Minecraft version provided contain invalid characters");
            continue;
        }
        break;
    }

    info!(
        "Minecraft version selected: {}",
        match minecraft.is_empty() {
            true => "latest",
            false => &minecraft,
        }
    );
    debug!("Installing fabric server...");
    match minecraft.is_empty() {
        true => subprocess_logger(
            "java",
            vec!["-jar", installer, "server", "-downloadMinecraft"],
        )
        .unwrap(),
        false => subprocess_logger(
            "java",
            vec![
                "-jar",
                installer,
                "server",
                "-mcversion",
                &minecraft,
                "-downloadMinecraft",
            ],
        )
        .unwrap(),
    };
    info!("Fabric server installation complete");
    fs::remove_file(installer).unwrap();

    return [String::from("fabric-server-launch"), minecraft];
}

/// Run function to each loader
fn loader_setup(loader: i8) -> [String; 2] {
    return match loader {
        1 => vanilla_loader(),
        2 => fabric_loader(),
        _ => panic!(),
    };
}

/// Return a string with the launch command
fn start_command(jar_name: String) -> String {
    format!("java -Xms1G -Xmx2G -jar {}.jar nogui", jar_name)
}

fn line_change(file: &str, line: usize, str: &str) {
    let mut data: Vec<String> = io::BufReader::new(fs::File::open(&file).unwrap())
        .lines()
        .map(|x| x.unwrap())
        .collect();
    match data.len() > line {
        true => data[line] = String::from(str),
        false => panic!(""),
    }
    let mut f = fs::File::create(&file).unwrap();
    for line in data {
        writeln!(f, "{}", line).unwrap();
    }
}

/// Function to install and configure MCDReforged
fn mcdr_setup(loader: i8, py_cmd: &str) -> Result<String, io::Error> {
    info!("Using MCDReforged");
    subprocess_logger(&py_cmd, vec!["-m", "mcdreforged", "init"]).unwrap();
    let mut path = current_dir()?;
    path.push("server");
    set_current_dir(path)?;
    let [jar_name, mc_version] = loader_setup(loader);
    set_current_dir("..")?;

    let cmd = format!("start_command: {}", start_command(jar_name));
    line_change("config.yml", 19, &cmd);

    let input: String = Input::new()
        .with_prompt(i("Set the nickname of the server owner? [Skip]"))
        .allow_empty(true)
        .interact_text()
        .unwrap();
    let nickanme = input.trim();
    if nickanme.is_empty() == false {
        info!("Nickname to set: {}", nickanme);
        line_change("permission.yml", 13, &format!("- {}", nickanme));
    }
    Ok(mc_version)
}

fn launch_scripts(cmd: String) {
    fn script(file: &str, content: Vec<&str>) {
        let mut f = fs::File::create(file).unwrap();
        for line in content {
            writeln!(f, "{}", line).unwrap();
        }
    }

    info!("Creating launch scripts...");
    script("start.bat", vec!["@echo off", &cmd, ""]);
    script("start.sh", vec![r"#!\bin\bash", &cmd, ""]);
    if consts::OS == "linux" {
        subprocess_logger("chmod", vec!["+x", "start.sh"]).unwrap();
    }
}

/// Create server launch scripts, version filter and try to start the server
fn post_setup(
    is_mcdr: bool,
    python: &str,
    jar_file: Option<String>,
    mc: String,
) -> Result<(), io::Error> {
    launch_scripts(match is_mcdr {
        true => format!("{} -m mcdreforged start", python),
        false => start_command(jar_file.unwrap()),
    });

    let tmp: Vec<i8> = mc.split(".").map(|x| x.parse::<i8>().unwrap()).collect();
    let major = tmp[1];
    let minor = match tmp.len() == 3 {
        true => tmp[2],
        false => 0,
    };
    let is_invalid = major < 7 || (major == 7 && minor < 10);
    if is_invalid {
        warn!("Minecraft version too old, EULA does't exists");
        return Ok(());
    }

    if simple_yes_no("Do you want to start the server and set EULA=true?", false) {
        info!("Starting the server for the first time...");
        warn!("May take some time...");

        if is_mcdr {
            line_change("config.yml", 77, "disable_console_thread: true");
        }
        match consts::OS {
            "windows" => subprocess_logger("./start.bat", Vec::new()).unwrap(),
            "linux" => subprocess_logger("./start.sh", Vec::new()).unwrap(),
            _ => panic!(),
        };
        info!("First time server start complete");
        if is_mcdr {
            line_change("config.yml", 77, "disable_console_thread: false");
            let mut path = current_dir()?;
            path.push("server");
            set_current_dir(path)?;
        }
        line_change("eula.txt", 2, "eula=true");
        info!("EULA set to true complete");
        return Ok(());
    }
    Ok(())
}

/// Main script function
fn main() -> Result<(), String> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Auto server script is starting up");
    let python = check_environment();
    mk_folder();
    let loader = server_mod_loader();
    if simple_yes_no("Do you want to use MCDR?", true) {
        let mc = mcdr_setup(loader, &python).unwrap();
        post_setup(true, &python, None, mc).unwrap();
    } else {
        let [minecraft_jar, minecraft_version] = loader_setup(loader);
        post_setup(false, &python, Some(minecraft_jar), minecraft_version).unwrap();
    }
    info!("Script done");
    Ok(())
}
