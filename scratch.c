#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>

#include <arpa/inet.h>
#include <netdb.h>
#include <netinet/in.h>
#include <openssl/hmac.h>
#include <openssl/sha.h>
#include <sys/socket.h>

#define AWS_CREDS_FILE ".aws/credentials"

struct awscreds {
    char *key;
    char *secret;
    char *region;
};

int getawscreds(struct awscreds *creds) {
    const char *home = getenv("HOME");

    char credspath[1024];
    int res = snprintf(
        credspath,
        sizeof(credspath),
        "%s/%s",
        home,
        AWS_CREDS_FILE
    );
    if(res < 0) {
        fprintf(stdout, "failed to build aws credentials filepath\n");
        return -1;
    }

    fprintf(stdout, "%s\n", credspath);

    FILE *f = fopen(credspath, "rb");
    if (f == NULL) {
        fprintf(stdout, "failed to read aws credentials file\n");
        return -1; 
    }

    fseek(f, 0, SEEK_END);
    long filesize = ftell(f);
    fseek(f, 0, SEEK_SET);

    char *buf = (char *) malloc(filesize + 1);
    if (buf == NULL) {
        fprintf(stdout, "failed to allocate memory to read aws credentials file\n");
        fclose(f);
        return -1; 
    }

    size_t bytesRead = fread(buf, 1, filesize, f);
    buf[bytesRead] = '\0';
    
    fclose(f);

    char key[256];
    char secret[256];
    // TODO get region or default

    char *line = strtok((char *) buf, "\n");
    while(line != NULL) {
        if(strstr(line, "aws_access_key_id") != NULL) {
            sscanf(line, "aws_access_key_id = %s", key);
        } else if(strstr(line, "aws_secret_access_key") != NULL) {
            sscanf(line, "aws_secret_access_key = %s", secret);
        }

        line = strtok(NULL, "\n");
    }

    // TODO check key/secret were found
    creds->key = strdup(key);
    creds->secret = strdup(secret);

    return 0;
}

void tohex(unsigned char *input, size_t len, char *output) {
    for (int i = 0; i < len; i++) {
        sprintf(output + (i * 2), "%02x", input[i]);
    }
    output[len * 2] = '\0';
}

void hmac_sha256(const char *key, const char *data, unsigned char *output) {
    #pragma GCC diagnostic push
    #pragma GCC diagnostic ignored "-Wdeprecated-declarations"

    unsigned int len = 32;
    HMAC_CTX *ctx = HMAC_CTX_new();
    HMAC_Init_ex(ctx, key, strlen(key), EVP_sha256(), NULL);
    HMAC_Update(ctx, (unsigned char*) data, strlen(data));
    HMAC_Final(ctx, output, &len);
    HMAC_CTX_free(ctx);

    #pragma GCC diagnostic pop
}

int awstime(char *timestamp, size_t bufsize) {
    size_t tssize = 20;

    if(bufsize < tssize) {
        fprintf(stderr, "buffer size to small for timestamp. must be at least: %zu\n", tssize);
        return -1;
    }

    time_t now = time(NULL);
    if(now ==((time_t) -1)) {
        fprintf(stderr, "failed to get current time\n");
        return -1;
    }

    struct tm *utc = gmtime(&now);
    if(!utc) {
        fprintf(stderr, "failed to convert time to utc\n");
        return -1;
    }

    if(strftime(timestamp, tssize, "%Y%m%dT%H%M%SZ", utc) == 0) {
        fprintf(stderr, "failed to format timestamp\n");
        return -1;
    }

    return 0;
}

int awsdate(char *timestamp, size_t bufsize) {
    size_t tssize = 9;

    if(bufsize < tssize) {
        fprintf(stderr, "buffer size to small for date. must be at least: %zu\n", tssize);
        return -1;
    }

    time_t now = time(NULL);
    if(now ==((time_t) -1)) {
        fprintf(stderr, "failed to get current time\n");
        return -1;
    }

    struct tm *utc = gmtime(&now);
    if(!utc) {
        fprintf(stderr, "failed to convert time to utc\n");
        return -1;
    }

    if(strftime(timestamp, tssize, "%Y%m%d", utc) == 0) {
        fprintf(stderr, "failed to format dat\n");
        return -1;
    }

    return 0;
}


int main() {
    struct awscreds creds;
    if(getawscreds(&creds) < 0) {
        fprintf(stderr, "failed to get aws creds\n");
        return -EIO;
    }  

    fprintf(stdout, "%s, %s\n", creds.key, creds.secret);

    int sock;
    if((sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP)) < 0) {
        fprintf(stderr, "failed to create socket\n");
        return -EIO;
    }

    // get hostname ip
    char *hostname = "fuse-tmp.s3.amazonaws.com";
    struct hostent *h;
    if((h = gethostbyname(hostname)) == NULL) {
        fprintf(stderr, "failed to get ip of hostname\n");
        return -EIO;
    }
    char *ip = inet_ntoa(*((struct in_addr *) h->h_addr_list[0]));

    // set sin_addr and sin_port
    struct sockaddr_in *remote = (struct sockaddr_in *) malloc(sizeof(struct sockaddr_in *));
    remote->sin_family = AF_INET;
    int res;
    if((res = inet_pton(AF_INET, ip, (void *) (&(remote->sin_addr.s_addr)))) <= 0) {
        fprintf(stderr, "failed to set sin_addr\n");
        return -EIO;
    }
    remote->sin_port = htons(80);

    // connect
    if(connect(sock, (struct sockaddr *) remote, sizeof(struct sockaddr)) < 0) {
        fprintf(stderr, "failed to connect\n");
        return -EIO;
    }

    // create date/times
    char timestamp[20]; 
    if(awstime(timestamp, sizeof(timestamp)) != 0) {
        fprintf(stderr, "failed to get aws time\n");
        return -EIO;
    }

    char date[20]; 
    if(awsdate(date, sizeof(date)) != 0) {
        fprintf(stderr, "failed to get aws date\n");
        return -EIO;
    }

    // create aws canonical request
   char canonical[1024];
    res = snprintf(
        canonical, 
        sizeof(canonical),
        "GET\n"
        "/\n"
        "encoding-type=url&list-type=2&prefix=\n"
        "host:fuse-tmp.s3.us-west-2.amazonaws.com\n"
        "x-amz-content-sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"
        "x-amz-date:%s\n"
        "\n"
        "host;x-amz-content-sha256;x-amz-date\n"
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        timestamp
    );
    if(res < 0) {
        fprintf(stderr, "failed to build aws canonical request\n");
        return -EIO;
    }

    fprintf(stdout, "%s\n\n\n", canonical);

    // hash/hex the canonical req
    unsigned char canonicalhash[SHA256_DIGEST_LENGTH];
    SHA256((unsigned char*) canonical, strlen(canonical), canonicalhash);

    char canonicalhex[SHA256_DIGEST_LENGTH * 2 + 1];
    tohex(canonicalhash, SHA256_DIGEST_LENGTH, canonicalhex);

    // create the string that needs signed
    char tosign[1024];
    snprintf(
        tosign, 
        sizeof(tosign),
        "AWS4-HMAC-SHA256\n"
        "%s\n"
        "%s/us-west-2/s3/aws4_request\n"
        "%s", 
        timestamp, 
        date,
        canonicalhex
    );

    fprintf(stdout, "%s\n\n\n", tosign);

    // create the signer
    char kdate[32], kregion[32], kservice[32], signer[32];
    // char *secret = "AWS4943GoXYVDw8U3yhu9IibyuTEISdlOWoLJ7mjG+sA";
    char secret[256];
    res = snprintf(
        secret,
        sizeof(secret),
        "AWS4%s",
        creds.secret
    );

    hmac_sha256(secret, date, (unsigned char*) kdate);
    hmac_sha256(kdate, "us-west-2", (unsigned char*) kregion);
    hmac_sha256(kregion, "s3", (unsigned char*) kservice);
    hmac_sha256(kservice, "aws4_request", (unsigned char*) signer);

    // hash/hex the string to sing using the signer
    unsigned char signedhash[SHA256_DIGEST_LENGTH];
    hmac_sha256(signer, tosign, signedhash);

    char signature[SHA256_DIGEST_LENGTH * 2 + 1];
    tohex(signedhash, SHA256_DIGEST_LENGTH, signature);

    // create the http req
    char req[1024];
    snprintf(
        req, 
        sizeof(req),
        "GET /?encoding-type=url&list-type=2&prefix= HTTP/1.1\r\n"
        "Host: fuse-tmp.s3.us-west-2.amazonaws.com\r\n"
        "x-amz-date: %s\r\n"
        "x-amz-content-sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\r\n"
        "Authorization: AWS4-HMAC-SHA256 Credential=%s/%s/us-west-2/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature=%s\r\n\r\n",
        timestamp,
        creds.key,
        date,
        signature
    );
    
    fprintf(stdout, "%s\n\n\n", req);

    // send the http req
    int sent = 0;
    while(sent < strlen(req)) {
        res = send(sock, req + sent, strlen(req) - sent, 0);
        if(res == -1) {
            fprintf(stderr, "failed to send request\n");
            return -EIO;
        }

        sent += res;
    }

    // receive the http response
    char *response = (char*) malloc(0);
	char BUF[BUFSIZ];
	size_t recived_len = 0;
	while((recived_len = recv(sock, BUF, BUFSIZ-1, 0)) > 0) {
        BUF[recived_len] = '\0';
		response = (char*) realloc(response, strlen(response) + strlen(BUF) + 1);
		sprintf(response, "%s%s", response, BUF);
	}

    fprintf(stdout, "%s\n", response);

    close(sock);
    free(creds.key);
    free(creds.secret);

    return 0;
}
