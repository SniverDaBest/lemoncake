>[!IMPORTANT]
> This is all subject to change!\
> Maybe #3 will become #1 at some point.

| Name | Description | ID (rax) | rdi | rsi | rdx | r10 | r8 | r9 |
| :---- | :---- | :---- | :---- | :---- | :---- | :---- | :---- | :---- |
| write | Writes to stdout. | 1 | Formatting style. 1 is Normal, 2 is Info, 3 is Warning, 4 is Error, and 5 is TODO | Pointer to text | Length of text | N/A | N/A | N/A |
| panic | Makes the kernel panic with a message. | 2 | Pointer to text | Length of text | N/A | N/A | N/A | N/A
| wait | Makes the kernel wait for X amount of milliseconds | 3 | Milliseconds | N/A | N/A | N/A | N/A | N/A