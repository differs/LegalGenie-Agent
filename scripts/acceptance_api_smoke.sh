#!/usr/bin/env bash
set -euo pipefail

API_BASE="${API_BASE:-http://127.0.0.1:8000}"

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "Missing dependency: ${cmd}" >&2
    exit 2
  fi
}

require_cmd curl
require_cmd jq

TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "${TMP_DIR}"; }
trap cleanup EXIT

HTTP_CODE=""
HTTP_BODY=""

http_get() {
  local path="$1"
  local token="${2:-}"

  local url="${API_BASE%/}${path}"
  local out="${TMP_DIR}/resp.json"

  if [[ -n "${token}" ]]; then
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" \
      -H "Authorization: Bearer ${token}" \
      "${url}")"
  else
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" "${url}")"
  fi

  HTTP_BODY="$(cat "${out}")"
}

http_get_discard_body() {
  local path="$1"
  local token="${2:-}"

  local url="${API_BASE%/}${path}"
  local out="${TMP_DIR}/resp.bin"

  if [[ -n "${token}" ]]; then
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" \
      -H "Authorization: Bearer ${token}" \
      "${url}")"
  else
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" "${url}")"
  fi

  # Avoid putting binary responses into shell variables.
  HTTP_BODY=""
}

http_json() {
  local method="$1"
  local path="$2"
  local token="${3:-}"
  local data="${4:-}"

  local url="${API_BASE%/}${path}"
  local out="${TMP_DIR}/resp.json"

  if [[ -n "${token}" ]]; then
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" \
      -X "${method}" \
      -H "Authorization: Bearer ${token}" \
      -H "Content-Type: application/json" \
      --data "${data}" \
      "${url}")"
  else
    HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" \
      -X "${method}" \
      -H "Content-Type: application/json" \
      --data "${data}" \
      "${url}")"
  fi

  HTTP_BODY="$(cat "${out}")"
}

http_upload_file() {
  local path="$1"
  local token="$2"
  local file_path="$3"
  local filename="$4"

  local url="${API_BASE%/}${path}"
  local out="${TMP_DIR}/resp.json"

  HTTP_CODE="$(curl -sS -o "${out}" -w "%{http_code}" \
    -X POST \
    -H "Authorization: Bearer ${token}" \
    -F "file=@${file_path};filename=${filename}" \
    "${url}")"
  HTTP_BODY="$(cat "${out}")"
}

fail() {
  local msg="$1"
  echo "FAIL: ${msg}" >&2
  echo "HTTP ${HTTP_CODE}" >&2
  if [[ -n "${HTTP_BODY}" ]]; then
    echo "${HTTP_BODY}" | head -c 4000 >&2
    echo >&2
  fi
  exit 1
}

assert_code() {
  local want="$1"
  local msg="$2"
  if [[ "${HTTP_CODE}" != "${want}" ]]; then
    fail "${msg} (expected ${want}, got ${HTTP_CODE})"
  fi
}

assert_jq() {
  local jq_args=()
  while [[ "$#" -gt 0 ]]; do
    if [[ "$1" == "--" ]]; then
      shift
      break
    fi
    jq_args+=("$1")
    shift
  done

  local expr="${1:-}"
  local msg="${2:-}"
  if [[ -z "${expr}" || -z "${msg}" ]]; then
    echo "internal error: assert_jq usage: assert_jq [jq_opts...] -- <expr> <msg>" >&2
    exit 2
  fi

  if ! echo "${HTTP_BODY}" | jq -e "${jq_args[@]}" "${expr}" >/dev/null 2>&1; then
    fail "${msg} (jq check failed: ${expr})"
  fi
}

note() {
  echo
  echo "==> $1"
}

pass() {
  echo "PASS: $1"
}

RUN_ID="$(date +%Y%m%d_%H%M%S)_${RANDOM}"
PASS="Password123"

note "Health Check (${API_BASE})"
http_get "/api/v1/health"
assert_code "200" "health"
pass "health"

note "Register 3 Users (owner/member/viewer)"
OWNER_USER="acc_owner_${RUN_ID}"
MEMBER_USER="acc_member_${RUN_ID}"
VIEWER_USER="acc_viewer_${RUN_ID}"

http_json "POST" "/api/v1/auth/register" "" "$(jq -nc --arg u "${OWNER_USER}" --arg e "${OWNER_USER}@example.com" --arg p "${PASS}" '{username:$u,email:$e,password:$p}')"
assert_code "200" "register owner"
OWNER_ID="$(echo "${HTTP_BODY}" | jq -r '.data.user.id')"
OWNER_TOKEN="$(echo "${HTTP_BODY}" | jq -r '.data.access_token')"
[[ -n "${OWNER_ID}" && "${OWNER_ID}" != "null" ]] || fail "register owner: missing id"
[[ -n "${OWNER_TOKEN}" && "${OWNER_TOKEN}" != "null" ]] || fail "register owner: missing token"

http_json "POST" "/api/v1/auth/register" "" "$(jq -nc --arg u "${MEMBER_USER}" --arg e "${MEMBER_USER}@example.com" --arg p "${PASS}" '{username:$u,email:$e,password:$p}')"
assert_code "200" "register member"
MEMBER_ID="$(echo "${HTTP_BODY}" | jq -r '.data.user.id')"
MEMBER_TOKEN="$(echo "${HTTP_BODY}" | jq -r '.data.access_token')"

http_json "POST" "/api/v1/auth/register" "" "$(jq -nc --arg u "${VIEWER_USER}" --arg e "${VIEWER_USER}@example.com" --arg p "${PASS}" '{username:$u,email:$e,password:$p}')"
assert_code "200" "register viewer"
VIEWER_ID="$(echo "${HTTP_BODY}" | jq -r '.data.user.id')"
VIEWER_TOKEN="$(echo "${HTTP_BODY}" | jq -r '.data.access_token')"

pass "users registered"

note "Create Case"
http_json "POST" "/api/v1/cases" "${OWNER_TOKEN}" "$(jq -nc --arg n "Acceptance Case ${RUN_ID}" '{name:$n,description:"api smoke"}')"
assert_code "200" "create case"
CASE_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"
[[ -n "${CASE_ID}" && "${CASE_ID}" != "null" ]] || fail "create case: missing id"
pass "case created: ${CASE_ID}"

note "Add Member + Viewer"
http_json "POST" "/api/v1/cases/${CASE_ID}/members" "${OWNER_TOKEN}" "$(jq -nc --arg uid "${MEMBER_ID}" '{user_id:$uid,role_in_case:"member"}')"
assert_code "200" "add member"
http_json "POST" "/api/v1/cases/${CASE_ID}/members" "${OWNER_TOKEN}" "$(jq -nc --arg uid "${VIEWER_ID}" '{user_id:$uid,role_in_case:"viewer"}')"
assert_code "200" "add viewer"
pass "members added"

note "Verify /members/me Role"
http_get "/api/v1/cases/${CASE_ID}/members/me" "${OWNER_TOKEN}"
assert_code "200" "owner /members/me"
assert_jq -- '.data.role_in_case=="owner"' "owner role_in_case"

http_get "/api/v1/cases/${CASE_ID}/members/me" "${MEMBER_TOKEN}"
assert_code "200" "member /members/me"
assert_jq -- '.data.role_in_case=="member"' "member role_in_case"

http_get "/api/v1/cases/${CASE_ID}/members/me" "${VIEWER_TOKEN}"
assert_code "200" "viewer /members/me"
assert_jq -- '.data.role_in_case=="viewer"' "viewer role_in_case"
pass "roles verified"

note "Timeline: Create Nodes + Tags Filter + Viewer Write Deny"
http_json "POST" "/api/v1/cases/${CASE_ID}/timeline/nodes" "${MEMBER_TOKEN}" "$(jq -nc '{title:"Tagged Node",description:"for tags filter",event_time:"2024-01-10",tags:["alpha","beta"]}')"
assert_code "200" "create tagged node"
NODE_TAGGED_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"

http_json "POST" "/api/v1/cases/${CASE_ID}/timeline/nodes" "${MEMBER_TOKEN}" "$(jq -nc '{title:"Alpha Only",event_time:"2024-01-11",tags:["alpha"]}')"
assert_code "200" "create alpha-only node"

http_get "/api/v1/cases/${CASE_ID}/timeline/nodes?tags=alpha,beta&page=1&page_size=50" "${VIEWER_TOKEN}"
assert_code "200" "list nodes with tags filter"
assert_jq --arg nid "${NODE_TAGGED_ID}" -- '.data.nodes | length==1 and .[0].id==$nid' "tags filter returns only tagged node"
pass "tags filter"

http_json "POST" "/api/v1/cases/${CASE_ID}/timeline/nodes" "${VIEWER_TOKEN}" "$(jq -nc '{title:"should fail",event_time:"2024-01-12"}')"
assert_code "403" "viewer cannot create node"
pass "viewer write denied (node create)"

note "Files: Upload + Link/Unlink Evidence + Parse (txt)"
NOTE_FILE="${TMP_DIR}/note.txt"
printf "hello acceptance %s\n" "${RUN_ID}" > "${NOTE_FILE}"

http_upload_file "/api/v1/cases/${CASE_ID}/files" "${MEMBER_TOKEN}" "${NOTE_FILE}" "note_${RUN_ID}.txt"
assert_code "200" "member upload file"
EVIDENCE_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"
[[ -n "${EVIDENCE_ID}" && "${EVIDENCE_ID}" != "null" ]] || fail "upload: missing evidence id"

http_upload_file "/api/v1/cases/${CASE_ID}/files" "${VIEWER_TOKEN}" "${NOTE_FILE}" "note_${RUN_ID}.txt"
assert_code "403" "viewer cannot upload file"
pass "file upload permissions"

http_json "POST" "/api/v1/timeline/nodes/${NODE_TAGGED_ID}/evidence" "${MEMBER_TOKEN}" "$(jq -nc --arg eid "${EVIDENCE_ID}" '{evidence_id:$eid,anchor_type:"page",anchor_data:{page_num:1}}')"
assert_code "200" "link evidence"
LINK_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"
[[ -n "${LINK_ID}" && "${LINK_ID}" != "null" ]] || fail "link evidence: missing link id"

http_get "/api/v1/timeline/nodes/${NODE_TAGGED_ID}" "${VIEWER_TOKEN}"
assert_code "200" "get node detail"
assert_jq --arg eid "${EVIDENCE_ID}" -- '.data.evidence_links | length==1 and .[0].evidence_id==$eid' "node has evidence link"

http_json "DELETE" "/api/v1/timeline/nodes/${NODE_TAGGED_ID}/evidence/${LINK_ID}" "${MEMBER_TOKEN}" "{}"
assert_code "200" "unlink evidence"

http_get "/api/v1/timeline/nodes/${NODE_TAGGED_ID}" "${VIEWER_TOKEN}"
assert_code "200" "get node detail after unlink"
assert_jq -- '.data.evidence_links | length==0' "node evidence_links cleared"
pass "evidence link/unlink"

http_json "POST" "/api/v1/files/${EVIDENCE_ID}/parse" "${MEMBER_TOKEN}" "{}"
assert_code "200" "parse file"

PARSE_OK="0"
for _ in $(seq 1 60); do
  http_get "/api/v1/files/${EVIDENCE_ID}" "${MEMBER_TOKEN}"
  if [[ "${HTTP_CODE}" != "200" ]]; then
    sleep 0.2
    continue
  fi
  status="$(echo "${HTTP_BODY}" | jq -r '.data.parse_status // empty' || true)"
  if [[ "${status}" != "processing" && -n "${status}" ]]; then
    PARSE_OK="1"
    break
  fi
  sleep 0.25
done
if [[ "${PARSE_OK}" != "1" ]]; then
  fail "parse did not finish (still processing)"
fi
pass "file parse finished"

note "Persons: Dedupe + Relationships + Merge (Case-local) + Viewer Deny"
http_json "POST" "/api/v1/cases/${CASE_ID}/persons" "${OWNER_TOKEN}" "$(jq -nc '{name:"Zhang San",role_type:"plaintiff"}')"
assert_code "200" "create person 1"
P1_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons" "${OWNER_TOKEN}" "$(jq -nc '{name:"Zhang San",role_type:"witness"}')"
assert_code "200" "create person 2 (dup by name)"
P2_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons" "${OWNER_TOKEN}" "$(jq -nc '{name:"Li Si",role_type:"other"}')"
assert_code "200" "create person 3"
P3_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"

http_get "/api/v1/cases/${CASE_ID}/persons/dedupe" "${VIEWER_TOKEN}"
assert_code "200" "viewer get dedupe"
assert_jq -- '.data.groups | any(.key=="name:zhang san" and (.persons|length>=2))' "dedupe group for same name"
pass "dedupe suggestions"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons/relationships" "${OWNER_TOKEN}" \
  "$(jq -nc --arg from "${P2_ID}" --arg to "${P3_ID}" '{from_person_id:$from,to_person_id:$to,rel_type:"knows",rel_detail:"met at work"}')"
assert_code "200" "create relationship"
REL_ID="$(echo "${HTTP_BODY}" | jq -r '.data.id')"

http_get "/api/v1/cases/${CASE_ID}/persons/relationships" "${VIEWER_TOKEN}"
assert_code "200" "viewer list relationships"
assert_jq -- '.data.total>=1 and (.data.relationships|length>=1)' "relationships list non-empty"
pass "relationships list"

http_get "/api/v1/cases/${CASE_ID}/persons/graph" "${VIEWER_TOKEN}"
assert_code "200" "viewer get persons graph"
assert_jq -- '.data.nodes | length>=3' "persons graph nodes >= 3"
assert_jq --arg from "${P2_ID}" --arg to "${P3_ID}" -- '
  .data.edges | any(.from_person_id==$from and .to_person_id==$to and .rel_type=="knows")
' "persons graph contains relationship edge"
pass "persons graph"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons/relationships" "${VIEWER_TOKEN}" \
  "$(jq -nc --arg from "${P1_ID}" --arg to "${P3_ID}" '{from_person_id:$from,to_person_id:$to,rel_type:"knows"}')"
assert_code "403" "viewer cannot create relationship"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons/merge" "${VIEWER_TOKEN}" \
  "$(jq -nc --arg s "${P2_ID}" --arg t "${P1_ID}" '{source_person_id:$s,target_person_id:$t}')"
assert_code "403" "viewer cannot merge persons"
pass "viewer write denied (relationships/merge)"

http_json "POST" "/api/v1/cases/${CASE_ID}/persons/merge" "${OWNER_TOKEN}" \
  "$(jq -nc --arg s "${P2_ID}" --arg t "${P1_ID}" '{source_person_id:$s,target_person_id:$t}')"
assert_code "200" "merge persons (case-local)"
assert_jq -- '.data.moved_links>=1' "merge moved_links>=1"
assert_jq -- '.data.moved_relationships>=1' "merge moved_relationships>=1"

http_get "/api/v1/cases/${CASE_ID}/persons?page=1&page_size=100" "${OWNER_TOKEN}"
assert_code "200" "list persons after merge"
assert_jq --arg sid "${P2_ID}" --arg tid "${P1_ID}" -- '
  (.data.persons | any(.id==$tid))
  and
  (.data.persons | any(.id==$sid) | not)
' "source person removed from case list"

http_get "/api/v1/cases/${CASE_ID}/persons/relationships" "${OWNER_TOKEN}"
assert_code "200" "list relationships after merge"
assert_jq --arg from "${P1_ID}" --arg to "${P3_ID}" -- '
  .data.relationships | any(.from_person_id==$from and .to_person_id==$to and .rel_type=="knows")
' "relationship moved to target"

http_get "/api/v1/cases/${CASE_ID}/persons/graph" "${OWNER_TOKEN}"
assert_code "200" "owner get persons graph after merge"
assert_jq --arg from "${P1_ID}" --arg to "${P3_ID}" -- '
  .data.edges | any(.from_person_id==$from and .to_person_id==$to and .rel_type=="knows")
' "persons graph edge moved to target"
pass "case-local merge"

note "History: Target History Endpoints"
wait_for_history() {
  local token="$1"
  local target_type="$2"
  local target_id="$3"
  local min_len="$4"

  for _ in $(seq 1 30); do
    http_get "/api/v1/logs/${target_type}/${target_id}/history" "${token}"
    if [[ "${HTTP_CODE}" == "200" ]]; then
      len="$(echo "${HTTP_BODY}" | jq -r '.data.history | length' || echo "0")"
      if [[ "${len}" =~ ^[0-9]+$ ]] && (( len >= min_len )); then
        return 0
      fi
    fi
    sleep 0.2
  done
  fail "history not available: ${target_type}/${target_id} (min_len=${min_len})"
}

wait_for_history "${OWNER_TOKEN}" "person" "${P1_ID}" 1
pass "person history"
wait_for_history "${OWNER_TOKEN}" "person_relationship" "${REL_ID}" 1
pass "relationship history"
wait_for_history "${OWNER_TOKEN}" "node" "${NODE_TAGGED_ID}" 1
pass "node history"

note "Exports: Evidence List"
http_get_discard_body "/api/v1/cases/${CASE_ID}/exports/evidence-list" "${OWNER_TOKEN}"
assert_code "200" "export evidence list"
pass "export evidence list (xlsx)"

echo
echo "ALL PASS: acceptance_api_smoke (${RUN_ID})"
