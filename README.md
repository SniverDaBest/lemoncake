>[!NOTE]
> This is the `new-bootloader-testing` branch. I'm trying to use the new version of the `bootloader` crate, instead of GRUB, because of UEFI compatibility reasons.

# Info
Lemoncake is a small OS, that was originally called `lemonade`. However, I have come to dislike that name, and dislike the rest of the codebase. So, it's been scrapped, and it's renamed.\
\
It's written in Rust, with a chance some code in other languages may be written eventually..\
\
[![](https://tokei.rs/b1/github/SniverDaBest/lemoncake)](https://github.com/SniverDaBest/lemoncake)

>[!WARNING]
> If you're having issues with running QEMU, try removing the `--enable-kvm` flag from the command in the build script.

# Dependencies
To build, you can run `cargo run` in the root project directory.\
\
The only dependencies you'll need are:
- cargo
- qemu
<!-- END OF LIST><!-->

# License
Lemoncake uses the BSD 2-clause (simplified) license. Check `LICENSE` for the full license.
