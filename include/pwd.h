#ifndef _PWD_H
#define _PWD_H

#include <sys/types.h>

#ifdef __APPLE__
struct passwd {
    char   *pw_name;
    char   *pw_passwd;
    uid_t   pw_uid;
    gid_t   pw_gid;
    time_t  pw_change;
    char   *pw_class;
    char   *pw_gecos;
    char   *pw_dir;
    char   *pw_shell;
    time_t  pw_expire;
};
#else
struct passwd {
    char   *pw_name;
    char   *pw_passwd;
    uid_t   pw_uid;
    gid_t   pw_gid;
    char   *pw_gecos;
    char   *pw_dir;
    char   *pw_shell;
};
#endif

struct passwd *getpwuid(uid_t uid);
struct passwd *getpwnam(const char *name);

#endif /* _PWD_H */
