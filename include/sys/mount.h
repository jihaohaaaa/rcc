#ifndef _SYS_MOUNT_H
#define _SYS_MOUNT_H

#include <sys/types.h>

#define MNT_RDONLY      0x00000001
#define MNT_SYNCHRONOUS 0x00000002
#define MNT_NOEXEC      0x00000004
#define MNT_NOSUID      0x00000008
#define MNT_NODEV       0x00000010
#define MNT_UNION       0x00000020
#define MNT_ASYNC       0x00000040
#define MNT_LOCAL       0x00001000

struct statfs {
    short   f_otype;
    short   f_oflags;
    long    f_bsize;
    long    f_iosize;
    long    f_blocks;
    long    f_bfree;
    long    f_bavail;
    long    f_files;
    long    f_ffree;
    unsigned long f_fsid;
    uid_t   f_owner;
    short   f_reserved1;
    short   f_type;
    unsigned int f_flags;
    long    f_reserved2[2];
    char    f_fstypename[16];
    char    f_mntonname[1024];
    char    f_mntfromname[1024];
    char    f_reserved3[8];
};

int statfs(const char *path, struct statfs *buf);
int fstatfs(int fd, struct statfs *buf);

#endif /* _SYS_MOUNT_H */
