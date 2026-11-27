# Security

For a vulnerability in Hexaris, use GitHub's private vulnerability reporting if it is enabled for the repository. Include the Hexaris version, platform, observed impact, and a minimal non-sensitive artifact or diagnostic description.

The analysis engine treats input binaries as data. The desktop and CLI do not execute binaries during analysis. Projects use bounded metadata and embedded bytes with a checksum. The checksum detects changes to the saved image and does not authenticate the project author.
