>[!NOTE]
> This is the `new-bootloader-testing` branch. I'm trying to use the new version of the `bootloader` crate, instead of GRUB, because of UEFI compatibility reasons.

# Info
Lemoncake is a small OS, that was originally called `lemonade`. However, I have come to dislike that name, and dislike the rest of the codebase. So, it's been scrapped, and it's renamed.\
\
It's written in Rust, with a chance some code in other languages may be written eventually..\
\
[![](https://tokei.rs/b1/github/SniverDaBest/lemoncake)](https://github.com/SniverDaBest/lemoncake)

>[!TIP]
> If you're having issues, try running the utility script.\
> You can do so with the following command: `python utils.py`\
> The script creates the hard disk image, and uses `rustup` to install some components and target info.

# Dependencies
To build, you can run `cargo build` in the root project directory.\
\
The only dependencies you'll need are:
- Cargo
- Qemu
- Python
<!-- END OF LIST><!-->

# Running it yourself
>[!CAUTION]
> It isn't recommended to run this on your host PC, as it could possibly break something that you need, wipe a drive, etc.

You can check it out in Qemu, by just running `cargo run` in the root project directory.\
If you want, you could also use VirtualBox, VMWare, or maybe something else to emulate it, but it won't happen with `cargo run`, you'd need to manually do that yourself.

>[!TIP]
> If you're having issues with running QEMU, try removing the `--enable-kvm` flag from the command in the build script.

# Contributions
Contributions would by highly appreciated!

# License
Lemoncake uses the BSD 2-clause (simplified) license. Check `LICENSE` for the full license.
