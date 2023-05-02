[View code on GitHub](https://github.com/metaplex-foundation/sugar/script/sugar-install.sh)

The `sugar` script automates the download and installation of the Sugar CLI binary for macOS, Linux, or other Unix-like OS running variations of the sh Unix shells. It performs platform detection, downloads the appropriate binary for the detected platform, and copies it to a folder in the PATH variable.

The script starts by defining helper functions for colored output and error handling. It then detects the OS flavor and processor architecture. If the processor is an unsupported ARM or PowerPC architecture and not macOS, the script exits with an error message.

Next, the script fetches the available releases from the GitHub repository and prompts the user to select a release version. It then determines the appropriate binary version based on the OS flavor and processor architecture.

The script creates a temporary directory, downloads the selected binary, and ensures it is executable. If the binary is already present on the system, the user is prompted to replace it. If not, the script determines a suitable directory for the binary, either `~/.cargo/bin` or `~/bin`, and moves the binary there.

If the binary is not in the PATH after installation, the script adds the target directory to the PATH variable in the user's shell configuration file (e.g., `~/.bashrc`). If the configuration file is not found, the user is prompted to create it.

Finally, the script performs a sanity check to ensure the binary is in the PATH and provides instructions for using the installed Sugar CLI binary.
## Questions: 
 1. **What platforms are supported by this installation script?**

   The script supports macOS, Linux, and other Unix-like operating systems running variations of the sh Unix shells.

2. **How does the script determine the latest release of Sugar CLI binary?**

   The script fetches the release information from the GitHub API and filters the tags to find the latest releases for both v1 and v2 versions.

3. **How does the script handle existing installations of Sugar CLI binary?**

   If an existing installation is found, the script prompts the user to replace the existing binary or cancel the operation. If the user chooses to replace it, the script moves the new binary to the existing location.