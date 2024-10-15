#define FUSE_USE_VERSION 31

#include <errno.h>
#include <fuse.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <arpa/inet.h>
#include <netdb.h>
#include <netinet/in.h>
#include <openssl/hmac.h>
#include <openssl/sha.h>
#include <sys/socket.h>

// #include <stdlib.h>
// #include <time.h>

#define KEEP_FILE ".keep"

char *BUCKET;

void removedir(int idx) {

}

int isdir(const char *path) {
    return 0;
}

void addfile(const char *path) {

}

void removefile(int idx) {

}

int isfile(const char *path) {
    return 0;
}

void writetofile(
    const char *path, 
    const char *content
) {

}

void to_hex(unsigned char *input, size_t len, char *output) {
    for (int i = 0; i < len; i++) {
        sprintf(output + (i * 2), "%02x", input[i]);
    }
    output[len * 2] = '\0';
}

void hmac_sha256(const char *key, const char *data, unsigned char *output) {
    unsigned int len = 32;
    HMAC_CTX *ctx = HMAC_CTX_new();
    HMAC_Init_ex(ctx, key, strlen(key), EVP_sha256(), NULL);
    HMAC_Update(ctx, (unsigned char*) data, strlen(data));
    HMAC_Final(ctx, output, &len);
    HMAC_CTX_free(ctx);
}

static int object_getattr(
    const char *path, 
    struct stat *stbuf, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`object_getattr` called for: %s\n", path);

    stbuf->st_uid = getuid();
    stbuf->st_gid = getgid();
    stbuf->st_atime = time(NULL);
    stbuf->st_mtime = time(NULL);
    if(strcmp(path, "/") == 0 || isdir(path)) {
        stbuf->st_mode = __S_IFDIR | 0755;
        stbuf->st_nlink = 2;
    } else if(isfile(path)) {
        stbuf->st_mode = __S_IFREG | 0644;
        stbuf->st_nlink = 1;
        stbuf->st_size = 1024;
    } else {
        return -ENOENT;
    }

    return 0;
}

static int object_readdir(
    const char *path, 
    void *buf, 
    fuse_fill_dir_t filler, 
    off_t offset, 
    struct fuse_file_info *fi, 
    enum fuse_readdir_flags flags
) {
    fprintf(stdout, "`object_readdir` called for: %s\n", path);


    return 0;
}

static int object_read(
    const char *path, 
    char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`object_read` called for: %s\n", path);

    // memcpy(buf, content + offset, size);

    return 0;
}

static int object_mkdir(
    const char *path, 
    mode_t mode
) {
    fprintf(stdout, "`object_mkdir` called for: %s\n", path);

    return 0;
}

static int object_rmdir(const char *path) {
    fprintf(stdout, "`object_rmdir` called for: %s\n", path);

    return 0;
}

static int object_mknod(
    const char *path, 
    mode_t mode, 
    dev_t rdev
) {
    fprintf(stdout, "`object_mknod` called for: %s\n", path);

    return 0;
}

static int object_unlink(const char *path) {
    fprintf(stdout, "`object_unlink` called for: %s\n", path);

    return 0;
}

static int object_write(
    const char *path, 
    const char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    fprintf(stdout, "`object_write` called for: %s\n", path);

    return size;
}

static const struct fuse_operations ops = {
    .getattr = object_getattr,
    .readdir = object_readdir,
    .read = object_read,
    .mkdir = object_mkdir,
    .rmdir = object_rmdir,
    .mknod = object_mknod,
    .unlink = object_unlink,
    .write = object_write
};

int main(int argc, char *argv[]) {
    return fuse_main(argc, argv, &ops, NULL);
}
