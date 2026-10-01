#ifndef _SIGNAL_H
#define _SIGNAL_H

#include <stddef.h>

typedef void (*sig_t)(int);
typedef sig_t sighandler_t;

#define SIG_DFL ((sig_t)0)
#define SIG_IGN ((sig_t)1)
#define SIG_ERR ((sig_t)-1)

#define SIGHUP    1
#define SIGINT    2
#define SIGQUIT   3
#define SIGILL    4
#define SIGTRAP   5
#define SIGABRT   6
#define SIGFPE    8
#define SIGKILL   9
#define SIGBUS    10
#define SIGSEGV   11
#define SIGSYS    12
#define SIGPIPE   13
#define SIGALRM   14
#define SIGTERM   15

sig_t signal(int sig, sig_t func);
int raise(int sig);
int kill(int pid, int sig);

#endif /* _SIGNAL_H */
