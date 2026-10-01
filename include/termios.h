#ifndef _TERMIOS_H
#define _TERMIOS_H

#include <sys/types.h>

typedef unsigned long tcflag_t;
typedef unsigned char cc_t;
typedef unsigned long speed_t;

#define NCCS 20

struct termios {
    tcflag_t c_iflag;
    tcflag_t c_oflag;
    tcflag_t c_cflag;
    tcflag_t c_lflag;
    cc_t     c_cc[NCCS];
    speed_t  c_ispeed;
    speed_t  c_ospeed;
};

#define TCSANOW   0
#define TCSADRAIN 1
#define TCSAFLUSH 2

#define IGNBRK 0x00000001
#define BRKINT 0x00000002
#define IGNPAR 0x00000004
#define PARMRK 0x00000008
#define INPCK  0x00000010
#define ISTRIP 0x00000020
#define INLCR  0x00000040
#define IGNCR  0x00000080
#define ICRNL  0x00000100
#define IXON   0x00000200
#define IXOFF  0x00000400

#define CSIZE  0x00000300
#define CS5    0x00000000
#define CS6    0x00000100
#define CS7    0x00000200
#define CS8    0x00000300
#define CSTOPB 0x00000400
#define CREAD  0x00000800
#define PARENB 0x00001000
#define PARODD 0x00002000

#define ECHO   0x00000008
#define ECHONL 0x00000010
#define ICANON 0x00000100
#define ISIG   0x00000080
#define IEXTEN 0x00000400
#define OPOST  0x00000001

#define VMIN  16
#define VTIME 17

int tcgetattr(int fd, struct termios *termios_p);
int tcsetattr(int fd, int optional_actions, const struct termios *termios_p);

#endif /* _TERMIOS_H */
