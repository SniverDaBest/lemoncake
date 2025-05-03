# SHFS
SHFS stands for *(and is a)* Super Horrible File System.

>[!WARNING]
>Do **NOT** use this outside of a testing environment. i.e. your work computer, a server, or your external SSD with all your projects.

Every partition with SHFS has a header at the beginning. Such header is as follows.

>[!NOTE]
> Field(s) with a `^` are volatile, and will change from time to time, and shouldn't be set/changed by the user/formatting utility, but rather the kernel.

| Field         | Type       |
| :------------ | :--------- |
| `signature`   | `[u8; 5]`  |
| `name`        | `[u8; 16]` |
| `rev`         | `u8`       |
| `piece_sz`    | `u16`      |
| `piece_count` | `u64`      |
| `free_space`^ | `u64`      |
| `index_end`   | `u64`      |

>[!WARNING]
>The example for `C/C++` *may* be incorrect! If it is, please make a PR/Issue on GitHub, and I can address it.

For those who code in `C/C++`, the header could possibly look like this:
```
#include <stdint.h>

typedef struct __attribute__((packed)) SHFS_header {
	char signature[5]; // signature
	char name[16]; // fs name
	uint8_t rev; // revision
	
	uint16_t piece_sz; // piece size
	uint64_t piece_count; // piece count
	
	uint64_t free_space; // free space
	uint64_t index_end; // end of the index
} SHFS_header;
```

However, if you code in `Rust`, it could possibly look like this:
```
#[repr(C, packed)]
pub struct SHFSHeader {
	/// signature
	signature: [u8; 5],
	/// fs name
	name: [u8; 16],
	/// revision
	rev: u8,
	
	/// piece size
	piece_sz: u16,
	/// piece count
	piece_count: u64,
	
	/// free space
	free_space: u64,
	/// end of the index
	index_end: u64,
}
```

The `signature` field should always store `SHFS!` As bytes (`*b”SHFS!”`)

>[!NOTE]
>If the partition name exceeds the 16 character limit, it will just be cut off.
>For example, a drive with name `I love kittens!! I hate pidgeons!!` (34 characters) will become `I love kittens!!` (16 characters)

The `name` field is for storing the name of the FS. Could be the partition name, or it could be something else.

The `rev` field is the revision of SHFS being used.

The `piece_sz` field is the size of each piece (in MB).

The `piece_count` field is self explanatory.

The `free_space` field is also self explanatory. However, do note that it stores the free space in bytes.

Finally, the `index_end` field is the address of where the index ends, and the next byte is of the FS.

## Wait, "index"? What is that?
The index is made to store the prefix and suffix of the most used files in the FS. It could be used by tools to find specific files quickly, instead of having to search all around the FS manually.

So the FS could look like:
```
/ <- root
/dir1/silly_cat.gif
[ snip ]
/dir4/important_docs.pdf
[ snip ]
/dir7/fan_speed_data.csv
/dir8/nyan_cat.gif
```

Where the index could store:
```
dir1/cat.gif
dir4/ocs.pdf
dir7/ata.csv
dir8/cat.gif
```

So hypothetically, a searching software could check the index when searching for `nyan_cat.gif`, and see both `dir1/cat.gif` and `dir8/cat.gif`, and check both of those places for `nyan_cat.gif`, instead of all 8+ directories. On a larger scale, this could possibly make major performance improvements for certain tasks, like searching the filesystem.

The OS or user can decide whether to make the index size big, small, or not even use it at all. They can also change the filename length from 12 (recommended) to something like 24. In some scenarios, it doesn't make sense to waste space on something that they wouldn't use at any point in time. Maybe a user has a tiny drive, and doesn't want any FS bloat. However, it can boost performance in some tasks.

But, it is **NOT** recommended to change the size of the index, since it would require a lot of manual work, and can corrupt your drive, cause loss of data, and many other horrible things that I highly doubt you want to happen.

## Why should I use SHFS instead of BTRFS, or Ext4?
You shouldn't. ...yet. It's currently an **extremely** new filesystem, and hasn't been tested much, if at all. As previously stated, do **NOT** use this outside of a testing environment. e.g., your work computer, a server, or your external SSD with all your projects.

# TL;DR
SHFS is a filesystem you should **NOT** use yet, as it's very new and untested. It has benefits for filesystem searching, and can be customized to fit user's needs.