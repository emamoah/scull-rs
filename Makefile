# SPDX-License-Identifier: GPL-2.0

SUBDIRS = scull

KDIR ?= /lib/modules/`uname -r`/build

all: subdirs

subdirs:
	for n in $(SUBDIRS); do $(MAKE) -C $$n || exit 1; done

clean:
	for n in $(SUBDIRS); do $(MAKE) -C $$n clean; done

modules_install:
	for n in $(SUBDIRS); do $(MAKE) -C $$n modules_install; done

rust-analyzer:
	$(MAKE) -C $(KDIR) M=$$PWD rust-analyzer
