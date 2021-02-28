import React from 'react';
import { Link as RouterLink, useParams } from 'react-router-dom';
import {
    Breadcrumb,
    BreadcrumbItem,
    BreadcrumbLink,
    Container,
    Text,
    Heading,
    CircularProgress,
    Center,
    Icon,
    Divider,
    Link,
} from '@chakra-ui/react';
import { ChevronRightIcon } from '@chakra-ui/icons';
import { FaFilm } from 'react-icons/fa';
import { useProgramQuery } from '../generated/graphql';
import { parseAndFormatDate } from '../utils';

type ProgramParams = {
    programId: string;
};

const Program = () => {
    const { programId } = useParams<ProgramParams>();
    const { loading, error, data } = useProgramQuery({ variables: { programId } });
    return (
        <Container maxW="container.lg" mt="1rem">
            <Breadcrumb spacing="8px" separator={<ChevronRightIcon color="gray.500" />} mb="1.25rem">
                <BreadcrumbItem as={RouterLink} to="/programs">
                    <BreadcrumbLink>番組一覧</BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbItem isCurrentPage>
                    <BreadcrumbLink>番組情報</BreadcrumbLink>
                </BreadcrumbItem>
            </Breadcrumb>
            {loading ? (
                <Center flexDirection="column">
                    <CircularProgress isIndeterminate color="blue.300" />
                    <Text mt="3">読み込み中……</Text>
                </Center>
            ) : error ? (
                <Text color="red.500">{JSON.stringify(error)}</Text>
            ) : (
                <>
                    <Heading mb="4">{data?.program?.name}</Heading>
                    <Text color="gray.500" mb="2">
                        {parseAndFormatDate(data?.program?.startAt)}
                        {' - '}
                        {data?.program?.service?.name}
                    </Text>
                    <Text>{data?.program?.description}</Text>
                    {data?.program?.extended?.map((ex) => (
                        <Text mt="2">
                            {ex?.key}
                            <br />
                            {ex?.value}
                        </Text>
                    ))}
                    <Divider my="4" />
                    <Heading size="sm">動画一覧</Heading>
                    {data?.program?.videos &&
                        data.program.videos.map((video) => (
                            <Text mt="2">
                                <Icon as={FaFilm} mr="2" />
                                <Link href={`/stream?id=${video.id}`} color="blue.500" target="_blank">
                                    {video.fileName}
                                </Link>{' '}
                                ({(parseInt(video.totalLength, 10) / 1024 / 1024).toFixed(1)} MB)
                            </Text>
                        ))}
                </>
            )}
        </Container>
    );
};

export default Program;
