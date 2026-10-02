#!/usr/bin/env python3

import re
import sys

ident = re.compile('[?_a-zA-Z][_a-zA-Z]*')


def datalog_to_markdown(dl, out):
    lines = dl.readlines()

    header_depth = 1
    comments = []
    code = []

    for line in lines:
        # An empty line ends the section
        if line.isspace():
            if comments or code:
                write_section(out, comments, code)
                comments = []
                code = []

            continue

        # Preprocessor directives
        if line.startswith('#'):
            continue

        # A comment
        if line.startswith('//'):
            assert len(code) == 0

            line = line.strip('/')
            line = line.removeprefix(' ')

            comments.append(line)

            old_len = len(line)
            leading_pound_signs = old_len - len(line.lstrip('#'))
            if leading_pound_signs:
                header_depth = leading_pound_signs

            continue

        # Add a section header for datalog declarations
        words = line.split()
        if words[0] == '.type' or words[0] == '.decl':
            title = ident.match(words[1]).group()
            # h = '#' * header_depth
            h = '####'
            comments = [f'{h} `{title}`\n', '\n'] + comments

        code.append(line)

    write_section(out, comments, code)


def write_section(out, comments, code):
    for line in comments:
        out.write(line)

    if comments and not comments[-1].isspace():
        out.write('\n')

    for line in code:
        out.write('\t')
        out.write(line)

    if code and not code[-1].isspace():
        out.write('\n')


if __name__ == '__main__':
    infile = sys.argv[1]
    with open(infile) as infile:
        datalog_to_markdown(infile, sys.stdout)

