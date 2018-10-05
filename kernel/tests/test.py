import argparse
import pexpect, sys, os
import json

cur_dir = os.path.dirname(os.path.realpath(__file__))
test_spec_file = open(os.path.join(cur_dir, "test-specs.json"), "r")

test_spec_obj = json.load(test_spec_file)
general_expected_lines = test_spec_obj["general-expected-lines"]
test_specs = test_spec_obj["tests"]

def expect_output_lines(proc, expected_lines):
    expected_lines = [pexpect.TIMEOUT, pexpect.EOF] + expected_lines
    while (len(expected_lines) > 2):
        index = proc.expect(expected_lines)
        if index == 0:
            proc.close()
            return "Timeout, or expected result not found"
        elif index == 1:
            proc.close()
            return "Unexpected EOF, maybe QEMU quitted unexpectedly"
        else:
            del expected_lines[index]
    return None

def run_qemu(qemu_cmd, spec, timeout): 
    proc = pexpect.spawn(qemu_cmd, timeout=timeout)
    proc.logfile = open(os.path.join(cur_dir, "%s.log" % spec["name"]), "w")

    p1 = expect_output_lines(proc, general_expected_lines)
    if p1:
        return p1

    proc.sendline(spec["name"])

    p2 = expect_output_lines(proc, spec["expected-lines"])
    return p2

    proc.close()
    return None
    
def run_test(qemu_cmd, user_prog_name, timeout):
    found = False
    for test_spec in test_specs:
        if (test_spec["name"] == user_prog_name):
            found = True

            print("Running test %s:" % (user_prog_name))
            result = run_qemu(qemu_cmd, test_spec, timeout)
            if result != None:
                print("Test %s failed: %s" % (user_prog_name, result))
            else:
                print("Test %s passed." % (user_prog_name))

    if not found:
        print("Test %s not found." % (user_prog_name))


parser = argparse.ArgumentParser(description='Runs user programs as tests')
parser.add_argument('--qemu', '-q', help='The console command starting QEMU', required=True)
parser.add_argument('--progs', '-p', help='Test names running', type=str, nargs='+', required=True)
parser.add_argument('--timeout', '-t', help='Allowed time for a test', type=int, default=10)

args = parser.parse_args()

qemu_cmd = args.qemu
user_prog_names = args.progs
timeout = args.timeout

for user_prog_name in user_prog_names:
    run_test(qemu_cmd, user_prog_name, timeout)

