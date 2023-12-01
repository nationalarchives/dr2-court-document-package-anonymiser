# Court document anonymiser

This repo is a [workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) with four members 

## Anonymiser script (lib)

This script takes production judgment packages from TRE and anonymises them with the following steps:

* It replaces Contact-Email and Contact-Name with XXXXXXX
* It generates a new docx file which only contains the name of the judgment.
* It updates the checksum field with the calculated checksum of the new docx file.
* It renames the folder and metadata file from TDR-xxx to TST-xxx.
* It creates a new tar.gz folder in the output directory. 
* It deletes the uncompressed folder in the output directory.

## Install
There is an [install.sh](./install.sh) script which will download the latest binary from GitHub and add it to `$HOME/.anonymiser/bin`.
```bash
curl https://raw.githubusercontent.com/nationalarchives/dr2-court-document-package-anonymiser/main/install.sh | sh
```
You will need to add `$HOME/.anonymiser/bin` to your $PATH.

## Running
```bash
anonymiser --input /path/to/input --output /path/to/output
```

## Running with docker
```bash
docker run -v /path/to/input:/input -v /path/to/output:/output public.ecr.aws/u4s1g5v1/anonymiser
```

The input path must only contain the tar.gz files you're converting.
