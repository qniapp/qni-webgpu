# Wait for the QNI WebGPU server to start.
echo "Waiting for QNI WebGPU server to open port ${port}..."
echo "TIMING - Starting wait at: $(date)"
if wait_until_port_used "${host}:${port}" 90; then
  echo "Discovered QNI WebGPU server listening on port ${port}!"
  echo "TIMING - Wait ended at: $(date)"
else
  echo "Timed out waiting for QNI WebGPU server to open port ${port}!" 1>&2
  echo "TIMING - Wait ended at: $(date)"
  pkill -P ${SCRIPT_PID}
  clean_up 1
fi
sleep 2
