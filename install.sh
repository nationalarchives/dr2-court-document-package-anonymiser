err="error attempting to download from the github repository"
check_cmd() {
	command -v "$1" >/dev/null 2>&1
}

download() {
	if check_cmd curl; then
		if ! (curl -fsSL "$1"); then
			echo err
			exit 1
		fi
	else
		if ! (wget -qO- "$1"); then
			echo err
			exit 1
		fi
	fi
}

mkdir -p $HOME/.anonymiser/bin
download https://github.com/nationalarchives/dr2-court-document-package-anonymiser/releases/latest/download/anonymiser > $HOME/.anonymiser/bin/anonymiser
chmod +x $HOME/.anonymiser/bin/anonymiser
echo "anonymiser installed at $HOME/.anonymiser/bin. Add this to your \$PATH"
