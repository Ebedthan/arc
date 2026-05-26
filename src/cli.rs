use clap::{crate_version, App, AppSettings, Arg};

pub fn build_app() -> App<'static, 'static> {
    let clap_color_setting = if std::env::var_os("NO_COLOR").is_none() {
        AppSettings::ColoredHelp
    } else {
        AppSettings::ColorNever
    };

    let app = App::new("arc")
        .version(crate_version!())
        .usage("sabreur [FLAGS/OPTIONS] <FILE>")
        .setting(clap_color_setting)
        .setting(AppSettings::DeriveDisplayOrder)
        .after_help(
            "Note: `arc -h` prints a short and concise overview while `arc --help` gives all \
                 details.",
        )
        .author("Anicet Ebou, anicet.ebou@gmail.com")
        .about("Fast archive converter")
        .arg(
            Arg::with_name("FILE")
                .help("Input file.")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("type")
                .long_help("File type to convert to")
                .short("t")
                .long("type")
                .takes_value(true)
                .possible_values(&["gz", "xz", "bz2"])
        )
        .arg(
            Arg::with_name("level")
                .help("Set the compression level")
                .long_help(
                    "Specifies the compression level wanted for output file:\n \
                        1: Level One, optimize the compression time\n \
                        2: Level Two\n \
                        3: Level Three\n \
                        4: Level Four\n \
                        5: Level Five\n \
                        6: Level Six\n \
                        7: Level Seven\n \
                        8: Level Eight\n \
                        9: Level Nine, optimize the size of the output\n",
                )
                .long("level")
                .short("l")
                .takes_value(true)
                .possible_values(&["1", "2", "3", "4", "5", "6", "7", "8", "9"])
                .hide_possible_values(true)
                .default_value("1"),
        )
        .arg(
            Arg::with_name("verbose")
                .long_help("Increases program logging verbosity each use for up to 3 times")
                .short("v")
                .long("verbose")
                .multiple(true),
        );

    app
}
