package tests

import (
	"bytes"
	"encoding/json"
	"io"
	"mime/multipart"
	"net/http"
	"net/http/cookiejar"
	"pergame/types"
	"strconv"
	"strings"
	"testing"
	"time"
)

const BaseURL = "http://localhost:8201/api"
const ADMIN_USERNAME = "Smaug"
const ADMIN_PASSWORD = "41258"
const USER_USERNAME = "Hacker"
const USER_PASSWORD = "41258"
const USER_ID = "543"

type APIResponse struct {
	Status     string          `json:"status"`
	Timestamp  types.Snowflake `json:"timestamp"`
	Message    string          `json:"message,omitempty"`
	Result     any             `json:"result,omitempty"`
	StatusCode int
}

func DoPost(t *testing.T, client *http.Client, path string, body any) APIResponse {
	var contentType string
	var reader io.Reader
	switch b := body.(type) {
	case map[string]any:
		jsonBody, err := json.Marshal(b)
		if err != nil {
			t.Fatalf("Erreur de marshal JSON: %v", err)
		}
		reader = bytes.NewReader(jsonBody)
		if contentType == "" {
			contentType = "application/json"
		}
	case string:
		reader = strings.NewReader(b)
		if contentType == "" {
			contentType = "application/x-www-form-urlencoded"
		}
	case nil:
		reader = nil
	default:
		t.Fatalf("Type de body non supporté: %T", body)
	}
	resp, err := client.Post(BaseURL+path, contentType, reader)
	if err != nil {
		t.Fatalf("POST %s failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

func DoGet(t *testing.T, client *http.Client, path string) APIResponse {
	resp, err := client.Get(BaseURL + path)
	if err != nil {
		t.Fatalf("GET %s failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

func DoDelete(t *testing.T, client *http.Client, path string) APIResponse {
	// Create request
	req, err := http.NewRequest(http.MethodDelete, BaseURL+path, nil)
	if err != nil {
		t.Fatalf("Error creating request: %v", err)
	}
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("DELETE %s failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

func DoPatch(t *testing.T, client *http.Client, path string, body any) APIResponse {
	var contentType string
	var reader io.Reader

	switch b := body.(type) {
	case map[string]any:
		jsonBody, err := json.Marshal(b)
		if err != nil {
			t.Fatalf("Erreur de marshal JSON: %v", err)
		}
		reader = bytes.NewReader(jsonBody)
		if contentType == "" {
			contentType = "application/json"
		}
	case string:
		reader = strings.NewReader(b)
		if contentType == "" {
			contentType = "application/x-www-form-urlencoded"
		}
	case nil:
		reader = nil
	default:
		t.Fatalf("Type de body non supporté: %T", body)
	}
	req, err := http.NewRequest(http.MethodPatch, BaseURL+path, reader)
	if err != nil {
		t.Fatalf("Error creating request: %v", err)
	}
	req.Header.Set("Content-Type", contentType)

	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("PATCH %s failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

func ParseJSONResponse[T any](t *testing.T, resp *http.Response) T {
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("Error during response reading: %v", err)
	}

	var data T

	// Ce bloc permet d’utiliser json.Number dans les maps dynamiques
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()

	if err := decoder.Decode(&data); err != nil {
		t.Fatalf("Error unmarshal JSON: %v\nBody:\n%s", err, body)
	}
	return data
}

// NewClient returns a fresh HTTP client with its own cookie jar (one client per authenticated user).
func NewClient(t *testing.T) *http.Client {
	jar, err := cookiejar.New(nil)
	if err != nil {
		t.Fatalf("Error with client init: %v", err)
	}
	return &http.Client{Jar: jar}
}

// DoPut performs a PUT request with a JSON body.
func DoPut(t *testing.T, client *http.Client, path string, body any) APIResponse {
	jsonBody, err := json.Marshal(body)
	if err != nil {
		t.Fatalf("Error marshaling JSON: %v", err)
	}
	req, err := http.NewRequest(http.MethodPut, BaseURL+path, bytes.NewReader(jsonBody))
	if err != nil {
		t.Fatalf("Error creating request: %v", err)
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := client.Do(req)
	if err != nil {
		t.Fatalf("PUT %s failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

// DoPostMultipart performs a multipart POST with a "file" part and extra form fields.
func DoPostMultipart(t *testing.T, client *http.Client, path, filename string, content []byte, fields map[string]string) APIResponse {
	var buf bytes.Buffer
	writer := multipart.NewWriter(&buf)
	for k, v := range fields {
		if err := writer.WriteField(k, v); err != nil {
			t.Fatalf("Error writing form field %s: %v", k, err)
		}
	}
	fileWriter, err := writer.CreateFormFile("file", filename)
	if err != nil {
		t.Fatalf("Error creating form file: %v", err)
	}
	if _, err := fileWriter.Write(content); err != nil {
		t.Fatalf("Error writing file content: %v", err)
	}
	if err := writer.Close(); err != nil {
		t.Fatalf("Error closing multipart writer: %v", err)
	}
	resp, err := client.Post(BaseURL+path, writer.FormDataContentType(), &buf)
	if err != nil {
		t.Fatalf("POST %s (multipart) failed: %v", path, err)
	}
	val := ParseJSONResponse[APIResponse](t, resp)
	val.StatusCode = resp.StatusCode
	return val
}

// TestUser is a freshly created user with its client already logged in.
type TestUser struct {
	ID       string
	Username string
	Password string
	Client   *http.Client
}

// SignupAndLogin creates a unique user (rerun-safe), logs it in and returns it with its own client.
func SignupAndLogin(t *testing.T, password string) TestUser {
	client := NewClient(t)
	suffix := strconv.FormatInt(time.Now().UnixNano(), 36)
	username := "test" + suffix
	r := DoPost(t, client, "/users/", map[string]any{
		"username":  username,
		"password":  password,
		"email":     username + "@example.com",
		"firstname": "Test",
		"lastname":  "User",
	})
	if r.Status != "success" {
		t.Fatalf("Signup failed (%d): %s", r.StatusCode, r.Message)
	}
	lr := LoginAs(t, client, username, password)
	if lr.Status != "success" {
		t.Fatalf("Login failed (%d): %s", lr.StatusCode, lr.Message)
	}
	me := DoGet(t, client, "/users/@me")
	if me.Status != "success" {
		t.Fatalf("Get @me failed (%d): %s", me.StatusCode, me.Message)
	}
	return TestUser{ID: ResultIDString(t, me), Username: username, Password: password, Client: client}
}

// ResultIDString extracts result.id, tolerating both string (Snowflake marshal) and json.Number decodings.
func ResultIDString(t *testing.T, r APIResponse) string {
	m, ok := r.Result.(map[string]any)
	if !ok {
		t.Fatalf("Result is not an object: %T", r.Result)
	}
	id, ok := AnyIDToString(m["id"])
	if !ok {
		t.Fatalf("Unexpected id type: %T", m["id"])
	}
	return id
}

// AnyIDToString converts a dynamic id value (string or json.Number) to its string form.
func AnyIDToString(v any) (string, bool) {
	switch id := v.(type) {
	case string:
		return id, true
	case json.Number:
		return id.String(), true
	default:
		return "", false
	}
}

// ResultArrayContainsID reports whether result is an array containing an object with the given id.
func ResultArrayContainsID(t *testing.T, r APIResponse, id string) bool {
	arr, ok := r.Result.([]any)
	if !ok {
		return false
	}
	for _, item := range arr {
		m, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if str, ok := AnyIDToString(m["id"]); ok && str == id {
			return true
		}
	}
	return false
}
