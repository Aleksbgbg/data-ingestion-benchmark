package main

import (
	"flag"
	"fmt"
	"io"
	"os"

	"github.com/gin-gonic/gin"
)

func main() {
	iface := flag.String("interface", "0.0.0.0", "Interface address to bind on")
	port := flag.Uint("port", 5003, "Port to bind on")

	flag.Parse()

	router := gin.Default()
	router.POST("/", ingest)
	router.Run(fmt.Sprintf("%s:%d", *iface, *port))
}

func ingest(c *gin.Context) {
	file, err := os.CreateTemp("", "go_ingest_file")
	if err != nil {
		panic(err)
	}
	defer os.Remove(file.Name())

	_, err = io.Copy(file, c.Request.Body)
	if err != nil {
		panic(err)
	}

	c.Status(204)
}
