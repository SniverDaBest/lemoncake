# Info
Lemoncake is a small OS that's written in Rust, which was originally called `lemonade`. However, I have come to dislike that name, and dislike the rest of the codebase. So, it's been scrapped, and it's renamed.\
\
[![](https://tokei.rs/b1/github/SniverDaBest/lemoncake)](https://github.com/SniverDaBest/lemoncake)

>[!WARNING]
> If you're having issues with running QEMU, try removing the `--enable-kvm` flag from the command in the build script.

# Dependencies
To build, you should probably use the `build` file in the root dir. It's a bash script, which will build, link, and do everything else for you.\
\
To build, you will need the following:
- nasm
- cargo
- qemu
- ovmf
<!-- END OF LIST><!-->

# Todo
- [X] Text rendering
- [ ] Get running normally working *(aka `cargo run`)*
- [ ] Optimize on memory *(i don't need to have all of those Strings and Vecs, could prob optimize them.)*
- [ ] Kernel

# License
Lemoncake uses the BSD 2-clause (simplified) license. Check `LICENSE` for the full license.
