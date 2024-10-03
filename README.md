# paperback

Yet another backup-to-paper solution. Stores about 1 kB per page at default
configuration (~1mm pixels).  Adjusting the pixel size is the most significant
factor in storage density.

Please see [sample output](sample.pdf) generated from the uncompressed `LICENSE`
file.  Note that this uses smaller-than-default pixel sizes.

## Why?
- You should probably use one of the [existing solutions](#see-also) instead of
  this thing.
- Instead of requiring specific key shards, restoring simply requires having
  enough readable sheets.  Somebody with an inadequate number of sheets should
  not be able to restore anything.

## Limitations
- The whole input file, plus all the recovery data, needs to be held in memory
  (possibly multiple times) for either create or restore.

## Usage

### Creating backups

Run `paperback create input.zip output.pdf`.  The input must be a single file;
if multiple files should be backed up together, put them into an archive.  The
file should be compressed; `paperback` does not attempt to do any compression.
If security would be useful, it would probably be a good idea to also encrypt
the file with a password.

The output contains the revision of `paperback` used, in case there are
incompatible changes in the future.  It also has the current page number, out of
the minimum required to restore, plus the number of extra pages for recovery.
Lastly, there is a document ID to help determine which set of backup the page
belongs to.

### Restoring backups

Scan the backups into a series of image files, one per page.  If the scan was a
PDF, use something like `pdftocairo` to convert it into images.
Run `paperback restore output.zip input.png input.png` etc. to restore.

You must have enough of the large QR codes (regardless of how many can be
recovered per page), plus at least one of the smaller, duplicated QR codes.

## License

The program as a whole is licensed under [AGPL-3.0-only](LICENSE).  This
contains the (unmodified) font metrics from Adobe for the 14 built-in fonts for
PDFs; that has the following license:

> This file and the 14 PostScript(R) AFM files it accompanies may be used,
copied, and distributed for any purpose and without charge, with or without
modification, provided that all copyright notices are retained; that the AFM
files are not distributed without this file; that all modifications to this file
or any of the AFM files are prominently noted in the modified file(s); and that
this paragraph is not modified. Adobe Systems has no responsibility or
obligation to support the use of the AFM files.

## See also
- https://github.com/cyphar/paperback
  - Need to manage key shards etc.
  - Single-page only
    - See also fork https://github.com/YouXam/paperarchive which does multiple
      pages.
- https://www.ollydbg.de/Paperbak/
  - Can't see to get this to work under Wine
- https://github.com/Wikinaut/paperback-cli
  - Outputs images instead of PDFs
- http://ronja.twibright.com/optar/
  - Source mirrored at https://github.com/colindean/optar
- https://github.com/za3k/qr-backup/
- https://github.com/intra2net/paperbackup
  - Focused on smaller files
- https://github.com/dmshaw/paperkey/
  - Focused on PGP keys specifically
- https://github.com/EmperorArthur/qrpdf
- https://github.com/jerabaul29/qrdump
- https://github.com/alisinabh/paperify
