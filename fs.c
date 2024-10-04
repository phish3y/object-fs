#define FUSE_USE_VERSION 31

#include <errno.h>
#include <fuse.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>


#define maxdirs 4
#define maxfiles 4

#define maxdirname 8
#define maxfilename 8
#define maxcontentsize 24

char dirlist[maxdirs][maxdirname];
int diridx = -1;

char filelist[maxfiles][maxfilename];
int fileidx = -1;

char contentlist[maxfiles][maxcontentsize];
int contentidx = -1;

void adddir(const char *dirname) {
    diridx++;
    strcpy(dirlist[diridx], dirname);
}

int isdir(const char *path) {
    path++;
    for(int i = 0; i <= diridx; i++) {
        if(strcmp(path, dirlist[i]) == 0) {
            return 1;
        }
    }

    return 0;
}

void addfile(const char *filename) {
    fileidx++;
    strcpy(filelist[fileidx], filename);

    contentidx++;
    strcpy(contentlist[contentidx], "");
}

int isfile(const char *path) {
    path++;
    for(int i = 0; i <= fileidx; i++) {
        if(strcmp(path, filelist[i]) == 0) {
            return 1;
        }
    }

    return 0;
}

int getfileidx(const char *path) {
    path++;
    for(int i = 0; i <= fileidx; i++) {
        if(strcmp(path, filelist[i]) == 0) {
            return i;
        }
    }

    return -1;
}

void writetofile(const char *path, const char *content) {
    int idx = getfileidx(path);
    if(idx == -1) {
        return;
    }

    strcpy(contentlist[idx], content);
}

static int e_getattr(
    const char *path, 
    struct stat *stbuf, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`e_getattr` called for: %s\n", path);

    stbuf->st_uid = getuid();
    stbuf->st_gid = getgid();
    stbuf->st_atime = time(NULL);
    stbuf->st_mtime = time(NULL);
    if(strcmp(path, "/") == 0 || isdir(path)) {
        fprintf(stdout, "is dir\n");
        stbuf->st_mode = __S_IFDIR | 0755;
        stbuf->st_nlink = 2;
    } else if(isfile(path)) {
        fprintf(stdout, "is file\n");
        stbuf->st_mode = __S_IFREG | 0644;
        stbuf->st_nlink = 1;
        stbuf->st_size = 1024;
    } else {
        fprintf(stdout, "is nothing\n");
        return -ENOENT;
    }

    return 0;
}

static int e_readdir(
    const char *path, 
    void *buf, 
    fuse_fill_dir_t filler, 
    off_t offset, struct fuse_file_info *fi, 
    enum fuse_readdir_flags flags
) {
    fprintf(stdout, "`e_readdir` called for: %s\n", path);

    filler(buf, ".", NULL, 0, 0);
    filler(buf, "..", NULL, 0, 0);
    if(strcmp(path, "/") == 0) {
        for(int i = 0; i <= diridx; i++) {
            filler(buf, dirlist[i], NULL, 0, 0);
        }
        for(int i = 0; i <= fileidx; i++) {
            filler(buf, filelist[i], NULL, 0, 0);
        }
    }
    
    return 0;
}

static int e_read(
    const char *path, 
    char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`e_read` called for: %s\n", path);

    int idx = getfileidx(path);
    if(idx == -1) {
        return -EINVAL;
    }

    char *content = contentlist[idx];

    memcpy(buf, content + offset, size);

    return strlen(content) - offset;
}

static int e_mkdir(const char *path, mode_t mode) {
    fprintf(stdout, "`e_mkdir` called for: %s\n", path);

    path++;
    adddir(path);

    return 0;
}

static int e_mknod(const char *path, mode_t mode, dev_t rdev) {
    fprintf(stdout, "`e_mknod` called for: %s\n", path);

    if(strlen(path) - 1 > maxfilename) {
        return -EINVAL;
    }

    path++;
    addfile(path);

    return 0;
}

static int e_write(
    const char *path, 
    const char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`e_write` called for: %s\n", path);

    if(strlen(path) - 1 > maxfilename) {
        return -EINVAL;
    }

    if(size + offset > maxcontentsize) {
        return -ENOMEM;
    }

    writetofile(path, buf);

    return size;
}

static const struct fuse_operations ops = {
    .getattr = e_getattr,
    .readdir = e_readdir,
    .read = e_read,
    .mkdir = e_mkdir,
    .mknod = e_mknod,
    .write = e_write
};

int main(int argc, char *argv[]) {
    return fuse_main(argc, argv, &ops, NULL);
}
