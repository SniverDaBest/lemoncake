# Info
Lemoncake is a small OS, that was originally called `lemonade`. However, I have come to dislike that name, and dislike the rest of the codebase. So, it's been scrapped, and it's renamed.\
\
It's written in Rust, with a teeny-tiny bit of assembly for booting, and other languages like C or C++ may be added in the future.\
It boots off of `GRUB2`, and doesn't support anything else... *yet*.\
It also needs `nasm` or `GAS` *(also known as `as`)* to build the assembly.\
\
[![](https://tokei.rs/b1/github/SniverDaBest/lemoncake)](https://github.com/SniverDaBest/lemoncake)

# Dependencies
To build, you should probably use the `build` file in the root dir. It's a bash script, which will build, link, and do everything else for you.\
\
To build, you will need the following:
- nasm
- grub2
    - probably some modules for it but idk which ones
- cargo
- xorriso
<!-- END OF LIST><!-->

# License
Lemoncake uses the BSD 2-clause (simplified) license. Check `LICENSE` for the full license.
