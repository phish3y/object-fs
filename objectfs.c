#define FUSE_USE_VERSION 31

#include <errno.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>

#include <arpa/inet.h>
#include <awsv4.h>
#include <aws.h>
#include <fuse.h>
#include <netinet/in.h>
#include <openssl/hmac.h>
#include <openssl/sha.h>
#include <sys/socket.h>

#define AWS_CREDS_FILE ".aws/credentials"
#define AWS_CONFIG_FILE ".aws/config"
#define KEEP_FILE ".keep"

#define BUCKET "fuse-tmp" // TODO
#define BUCKET_HOST "fuse-tmp.s3.amazonaws.com" // TODO

#define info(fmt, ...) fprintf(stdout, "INFO: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)
#define error(fmt, ...) fprintf(stderr, "ERROR: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)

#ifdef DEBUG
    #define debug(fmt, ...) fprintf(stderr, "DEBUG: %s:%d: " fmt, __FILE__, __LINE__, ##__VA_ARGS__)
#else
    #define debug(fmt, ...)
#endif

int bucketconnect() {
    int sock;
    if((sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)) < 0) {
        error("failed to create socket\n");
        return -1;
    }

    // get hostname ip
    struct hostent *h;
    if((h = gethostbyname(BUCKET_HOST)) == NULL) {
        error("failed to get ip of hostname\n");
        return -1;
    }
    char *ip = inet_ntoa(*((struct in_addr *) h->h_addr_list[0]));

    // set sin_addr and sin_port
    struct sockaddr_in *remote = (struct sockaddr_in *) malloc(sizeof(struct sockaddr_in *));
    remote->sin_family = AF_INET;
    int res;
    if((res = inet_pton(AF_INET, ip, (void *) (&(remote->sin_addr.s_addr)))) <= 0) {
        error("failed to set sin_addr\n");
        return -1;
    }
    remote->sin_port = htons(80);

    // connect
    if(connect(sock, (struct sockaddr *) remote, sizeof(struct sockaddr)) < 0) {
        error("failed to connect to bucket\n");
        return -1;
    }

    return sock;
}

int httpsend(const int sock, const char *req) {
    int sent = 0;
    while(sent < strlen(req)) {
        int res = send(sock, req + sent, strlen(req) - sent, 0);
        if(res == -1) {
            error("failed to send http request\n");
            return -1;
        }

        sent += res;
    }

    return 0;
}

int httpreceive(const int sock, char *output, const size_t len) {
    size_t totalrec = 0;
    char buf[BUFSIZ];
    ssize_t currentrec = 0;
    while((currentrec = recv(sock, buf, BUFSIZ - 1, 0)) > 0) {
        buf[currentrec] = '\0';
        
        // TODO resize output
        // if()

        memcpy(output + totalrec, buf, currentrec + 1);
        totalrec += currentrec;
    }

    if(currentrec < 0) {
        error("failed to receive http response");
        return -1;
    }

    return 0;
}

int getuntil(char *output, const size_t len, const char *input, const char *until) {
    if(output == NULL) {
        error("output buffer must not be null\n");
        return -1;
    }

    const char *pos = strstr(input, until);
    if (pos != NULL) {
        size_t sublen = pos - input;
        if (sublen >= len) {
            error("buffer size too small for substring. must be at least: %zu\n", sublen);
            return -1;
        }

        strncpy(output, input, sublen);
        output[sublen] = '\0';
    } else {
        strncpy(output, input, len - 1);
        output[len - 1] = '\0';
    }

    return 0;
}

static int object_getattr(
    const char *path, 
    struct stat *stbuf, 
    struct fuse_file_info *fi
) {
    debug("`object_getattr` called for: %s\n", path);

    // stbuf->st_uid = getuid();
    // stbuf->st_gid = getgid();
    // stbuf->st_atime = time(NULL);
    // stbuf->st_mtime = time(NULL);
    // if(strcmp(path, "/") == 0 || isdir(path)) {
    //     stbuf->st_mode = __S_IFDIR | 0755;
    //     stbuf->st_nlink = 2;
    // } else if(isfile(path)) {
    //     stbuf->st_mode = __S_IFREG | 0644;
    //     stbuf->st_nlink = 1;
    //     stbuf->st_size = 1024;
    // } else {
    //     return -ENOENT;
    // }

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
    debug("`object_readdir` called for: %s\n", path);

    return 0;
}

static int object_read(
    const char *path, 
    char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    debug("`object_read` called for: %s\n", path);

    // memcpy(buf, content + offset, size);

    return 0;
}

static int object_mkdir(
    const char *path, 
    mode_t mode
) {
    debug("`object_mkdir` called for: %s\n", path);

    return 0;
}

static int object_rmdir(const char *path) {
    debug("`object_rmdir` called for: %s\n", path);

    return 0;
}

static int object_mknod(
    const char *path, 
    mode_t mode, 
    dev_t rdev
) {
    debug("`object_mknod` called for: %s\n", path);

    return 0;
}

static int object_unlink(const char *path) {
    debug("`object_unlink` called for: %s\n", path);

    return 0;
}

static int object_write(
    const char *path, 
    const char *buf, 
    size_t size, 
    off_t offset, 
    struct fuse_file_info *fi
) {
    debug("`object_write` called for: %s\n", path);

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
    struct awscreds creds;
    if(getawscreds(&creds) < 0) {
        error("failed to get aws creds\n");
        return -1;
    }  

    struct awsconfig config;
    if(getawsconfig(&config) < 0) {
        error("failed to get aws config\n");
        return -1;
    }

    debug("aws info:\nAWS key: %s\nAWS secret: %s\nAWS region: %s\n", creds.key, creds.secret, config.region);

    int sock = bucketconnect();
    if(sock < 0) {
        error("failed to connect to bucket\n");
        return -1;
    }

    // create date/times
    char timestamp[20]; 
    if(awstime(timestamp, sizeof(timestamp)) != 0) {
        error("failed to get aws time\n");
        return -1;
    }

    char date[20]; 
    if(awsdate(date, sizeof(date)) != 0) {
        error("failed to get aws date\n");
        return -1;
    }

    char *payload = "";
    char payloadhex[HEX_LEN];
    if(sha256hex(payloadhex, sizeof(payloadhex), payload) != 0) {
        error("failed to sha256 hex the payload\n");
        return -1;  
    }

    // build canonical request
    char canonical[BUFSIZ];
    if(getcanonicalreq(
        canonical, 
        sizeof(canonical), 
        "GET", 
        BUCKET, 
        config.region, 
        payloadhex, 
        timestamp
    )) {
        error("failed to get canonical req\n");
        return -1;
    }

    debug("canonical:\n%s\n", canonical);

    char canonicalhex[HEX_LEN];
    if(sha256hex(canonicalhex, sizeof(canonicalhex), canonical) != 0) {
        error("failed to sha256 hex the canonical request\n");
        return -1;  
    }

    char tosign[BUFSIZ];
    if(getstringtosign(
        tosign,
        sizeof(tosign),
        timestamp,
        date,
        config.region,
        canonicalhex
    ) != 0) {
        error("failed to get string to sign\n");
        return -1;  
    }

    debug("to sign:\n%s\n", tosign);

    char signature[HEX_LEN];
    if(createsignature(
        signature,
        sizeof(signature),
        tosign,
        creds.secret,
        date,
        config.region
    ) != 0) {
        error("failed to create signature\n");
        return -1;  
    }
   
    // create the http req
    char req[1024];
    snprintf(
        req, 
        sizeof(req),
        "GET /?encoding-type=url&list-type=2&prefix= HTTP/1.1\r\n"
        "Host: %s.s3.%s.amazonaws.com\r\n"
        "x-amz-date: %s\r\n"
        "x-amz-content-sha256: %s\r\n"
        "Authorization: AWS4-HMAC-SHA256 Credential=%s/%s/%s/s3/aws4_request,SignedHeaders=host;x-amz-content-sha256;x-amz-date,Signature=%s\r\n\r\n",
        BUCKET,
        config.region,
        timestamp,
        payloadhex,
        creds.key,
        date,
        config.region,
        signature
    );
    
    debug("http req:\n%s\n", req);

    if(httpsend(sock, req) != 0) {
        error("failed to send http req\n");
        return -1;  
    }

    char response[BUFSIZ];
    if(httpreceive(sock, response, sizeof(response)) != 0) {
        error("failed to receive http res\n");
        return -1;  
    }

    debug("http res:\n%s\n", response);

    char sub[BUFSIZ];
    if(getuntil(sub, sizeof(sub), response, "\r\n") != 0) {
        error("failed to parse response\n");
        return -1;  
    }

    info("%s\n", sub);

    close(sock);
    free(creds.key);
    free(creds.secret);
    free(config.region);

    return 0;
    // return fuse_main(argc, argv, &ops, NULL);
}
